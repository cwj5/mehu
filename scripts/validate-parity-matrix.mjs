import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const DEFAULT_MATRIX_PATH = "plot3d_com_file/parity_matrix.json";

const REQUIRED_CAPABILITIES = [
    "READ",
    "FUNCTION",
    "VIEW",
    "VPOINT",
    "MINMAX",
    "CONTOURS",
    "PLOT",
    "WALLS",
    "SUBSETS",
    "FSURFACE",
    "TEXT",
    "SHOW"
];

const ALLOWED_STATES = new Set([
    "supported",
    "script-only",
    "gui-only",
    "not-supported"
]);

const TICKET_REFERENCE_PATTERN = /^TKT-\d{3}[A-Z]?$/;

const CAPABILITY_AFFECTING_FILES = [
    "plot3d_com_file/capability_catalog.md",
    "src-tauri/src/plot_state.rs",
    "src-tauri/src/com_parser.rs",
    "src-tauri/src/function_mapping.rs",
    "src-tauri/src/script_executor.rs",
    "src-tauri/src/lib.rs",
    "src/App.tsx",
    "src/components/Viewer3D.tsx",
    "src/utils/solutionData.ts"
];

const matrixPath = path.resolve(process.argv[2] ?? DEFAULT_MATRIX_PATH);

function fail(message) {
    console.error(`[parity-matrix] ${message}`);
    process.exit(1);
}

function formatPathForDisplay(filePath) {
    return path.relative(process.cwd(), filePath) || path.basename(filePath);
}

function addIssue(issues, message, fix) {
    issues.push(fix ? `${message} Fix: ${fix}` : message);
}

function isNonEmptyString(value) {
    return typeof value === "string" && value.trim().length > 0;
}

function parseIsoDate(value) {
    if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(value)) {
        return null;
    }

    const parsed = new Date(`${value}T00:00:00Z`);
    if (Number.isNaN(parsed.getTime())) {
        return null;
    }

    return parsed.toISOString().slice(0, 10) === value ? value : null;
}

function runGit(args) {
    return execFileSync("git", args, {
        cwd: process.cwd(),
        encoding: "utf8",
        stdio: ["ignore", "pipe", "pipe"]
    }).trimEnd();
}

function ensureGitRepository(issues) {
    try {
        const insideWorkTree = runGit(["rev-parse", "--is-inside-work-tree"]);
        if (insideWorkTree !== "true") {
            addIssue(
                issues,
                "freshness check requires running inside a git work tree",
                "Run the validator from a git checkout so capability history can be inspected."
            );
            return false;
        }

        return true;
    } catch (error) {
        addIssue(
            issues,
            `freshness check could not query git metadata (${error.message})`,
            "Ensure git is installed and the repository history is available locally and in CI."
        );
        return false;
    }
}

function getDirtyTrackedPaths(pathsToCheck) {
    const output = runGit(["status", "--porcelain", "--", ...pathsToCheck]);
    if (!output) {
        return [];
    }

    return output
        .split(/\r?\n/)
        .map((line) => line.slice(3).trim())
        .filter(Boolean);
}

function getLatestCapabilityChange() {
    const output = runGit([
        "--no-pager",
        "log",
        "-n",
        "1",
        "--date=short",
        "--pretty=format:%cs%x09%h%x09%s",
        "--name-only",
        "--",
        ...CAPABILITY_AFFECTING_FILES
    ]);

    if (!output) {
        return null;
    }

    const [metadataLine, ...fileLines] = output.split(/\r?\n/).filter(Boolean);
    const [date, commit, subject] = metadataLine.split("\t");

    return {
        date,
        commit,
        subject,
        file: fileLines[0] ?? "(unknown file)"
    };
}

function validateFreshness(issues, lastUpdated, matrixFilePath) {
    if (!ensureGitRepository(issues)) {
        return;
    }

    const pathsToInspect = [formatPathForDisplay(matrixFilePath), ...CAPABILITY_AFFECTING_FILES];

    let dirtyPaths;
    try {
        dirtyPaths = getDirtyTrackedPaths(pathsToInspect);
    } catch (error) {
        addIssue(
            issues,
            `freshness check could not inspect local file modifications (${error.message})`,
            "Resolve git status issues, then re-run the validator."
        );
        return;
    }

    const matrixRelativePath = formatPathForDisplay(matrixFilePath);
    const matrixDirty = dirtyPaths.includes(matrixRelativePath);
    const dirtyCapabilityPaths = dirtyPaths.filter((filePath) => filePath !== matrixRelativePath);
    if (dirtyCapabilityPaths.length > 0 && !matrixDirty) {
        addIssue(
            issues,
            `capability-affecting files are modified locally but ${matrixRelativePath} was not updated (${dirtyCapabilityPaths.join(", ")})`,
            `Review parity impact, then update ${matrixRelativePath} lastUpdated and any affected capability rows in the same change.`
        );
    }

    let latestChange;
    try {
        latestChange = getLatestCapabilityChange();
    } catch (error) {
        addIssue(
            issues,
            `freshness check could not read capability history (${error.message})`,
            "Ensure CI fetches sufficient git history before running the validator."
        );
        return;
    }

    if (!latestChange) {
        addIssue(
            issues,
            "freshness check found no capability-affecting history to compare against",
            "Confirm the validator's capability-affecting file list matches the repository layout."
        );
        return;
    }

    if (latestChange.date > lastUpdated) {
        addIssue(
            issues,
            `lastUpdated ${lastUpdated} is stale; latest capability-affecting change is ${latestChange.date} in ${latestChange.file} (${latestChange.commit} ${latestChange.subject})`,
            `After reviewing parity status, set lastUpdated in ${matrixRelativePath} to ${latestChange.date} or later.`
        );
    }
}

function printIssuesAndExit(issues) {
    if (issues.length === 0) {
        return;
    }

    console.error(`[parity-matrix] Validation failed with ${issues.length} issue(s):`);
    for (const [index, issue] of issues.entries()) {
        console.error(`[parity-matrix] ${index + 1}. ${issue}`);
    }
    process.exit(1);
}

if (!fs.existsSync(matrixPath)) {
    fail(`Missing required file: ${matrixPath}`);
}

let raw;
try {
    raw = fs.readFileSync(matrixPath, "utf8");
} catch (error) {
    fail(`Unable to read matrix file: ${error.message}`);
}

let matrix;
try {
    matrix = JSON.parse(raw);
} catch (error) {
    fail(`Invalid JSON: ${error.message}`);
}

const issues = [];

if (matrix.schemaVersion !== 1) {
    addIssue(
        issues,
        "schemaVersion must be 1",
        "Set schemaVersion to 1."
    );
}

const lastUpdated = parseIsoDate(matrix.lastUpdated);
if (!lastUpdated) {
    addIssue(
        issues,
        `lastUpdated must be a valid ISO date (YYYY-MM-DD), received ${JSON.stringify(matrix.lastUpdated)}`,
        "Set lastUpdated to the parity review date, for example 2026-03-27."
    );
}

if (!Array.isArray(matrix.parityStates)) {
    addIssue(
        issues,
        "parityStates must be an array",
        "Restore the canonical parityStates array from capability_catalog.md."
    );
} else {
    const states = matrix.parityStates;
    const uniqueStates = new Set(states);
    if (uniqueStates.size !== states.length) {
        addIssue(
            issues,
            "parityStates contains duplicate entries",
            "Keep each allowed parity state exactly once."
        );
    }

    const unknownStates = states.filter((state) => !ALLOWED_STATES.has(state));
    if (unknownStates.length > 0) {
        addIssue(
            issues,
            `parityStates contains unknown values: ${unknownStates.join(", ")}`,
            "Use only supported, script-only, gui-only, and not-supported."
        );
    }

    const missingStates = [...ALLOWED_STATES].filter((state) => !uniqueStates.has(state));
    if (missingStates.length > 0) {
        addIssue(
            issues,
            `parityStates is missing required values: ${missingStates.join(", ")}`,
            "Restore the full canonical parity state set."
        );
    }
}

if (!Array.isArray(matrix.capabilities)) {
    addIssue(
        issues,
        "capabilities must be an array",
        "Restore the capabilities array with one row per canonical capability."
    );
}

if (!Array.isArray(matrix.outOfScopeCommands)) {
    addIssue(
        issues,
        "outOfScopeCommands must be an array",
        "Restore the explicit outOfScopeCommands list."
    );
}

const idsSeen = new Set();
if (Array.isArray(matrix.capabilities)) {
    for (const [index, capability] of matrix.capabilities.entries()) {
        const rowLabel = `capabilities[${index}]`;

        if (!capability || typeof capability !== "object") {
            addIssue(
                issues,
                `${rowLabel} must be an object`,
                "Replace the row with an object containing id, status, ticket, notes, and optional rationale."
            );
            continue;
        }

        if (!isNonEmptyString(capability.id)) {
            addIssue(
                issues,
                `${rowLabel}.id must be a non-empty string`,
                "Use one of the canonical capability IDs from capability_catalog.md."
            );
            continue;
        }

        if (idsSeen.has(capability.id)) {
            addIssue(
                issues,
                `duplicate capability id: ${capability.id}`,
                "Keep exactly one row per capability ID."
            );
        }
        idsSeen.add(capability.id);

        if (!isNonEmptyString(capability.status) || !ALLOWED_STATES.has(capability.status)) {
            addIssue(
                issues,
                `${rowLabel} (${capability.id}) has invalid status ${JSON.stringify(capability.status)}`,
                "Use supported, script-only, gui-only, or not-supported."
            );
        }

        if (!isNonEmptyString(capability.ticket)) {
            addIssue(
                issues,
                `${rowLabel} (${capability.id}) is missing a ticket reference`,
                "Set ticket to the owning work item, for example TKT-009 or TKT-012A."
            );
        } else if (!TICKET_REFERENCE_PATTERN.test(capability.ticket.trim())) {
            addIssue(
                issues,
                `${rowLabel} (${capability.id}) has invalid ticket reference ${JSON.stringify(capability.ticket)}`,
                "Use ticket format TKT-### or TKT-###A."
            );
        }

        if (!isNonEmptyString(capability.notes)) {
            addIssue(
                issues,
                `${rowLabel} (${capability.id}) must include non-empty notes`,
                "Add a concise notes string describing current parity status or the remaining gap."
            );
        }

        if (
            (capability.status === "script-only" || capability.status === "gui-only") &&
            !isNonEmptyString(capability.rationale)
        ) {
            addIssue(
                issues,
                `${rowLabel} (${capability.id}) requires non-empty rationale when status is ${capability.status}`,
                `Add rationale explaining why ${capability.status} is acceptable for now and what blocks full parity.`
            );
        }

        if (
            capability.rationale !== undefined &&
            typeof capability.rationale === "string" &&
            capability.rationale.trim().length === 0
        ) {
            addIssue(
                issues,
                `${rowLabel} (${capability.id}) has an empty rationale string`,
                "Remove rationale or replace it with meaningful text."
            );
        }
    }
}

for (const requiredId of REQUIRED_CAPABILITIES) {
    if (!idsSeen.has(requiredId)) {
        addIssue(
            issues,
            `missing required capability id: ${requiredId}`,
            `Add a row for ${requiredId} to plot3d_com_file/parity_matrix.json.`
        );
    }
}

const unknownRequired = [...idsSeen].filter((id) => !REQUIRED_CAPABILITIES.includes(id));
if (unknownRequired.length > 0) {
    addIssue(
        issues,
        `unknown capability ids found: ${unknownRequired.join(", ")}`,
        "Restrict capability rows to the canonical IDs listed in capability_catalog.md."
    );
}

if (lastUpdated) {
    validateFreshness(issues, lastUpdated, matrixPath);
}

printIssuesAndExit(issues);

console.log(`[parity-matrix] OK: ${matrix.capabilities.length} capability rows validated`);
