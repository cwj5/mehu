import fs from "node:fs";
import path from "node:path";

const matrixPath = path.resolve("plot3d_com_file/parity_matrix.json");

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

function fail(message) {
    console.error(`[parity-matrix] ${message}`);
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

if (matrix.schemaVersion !== 1) {
    fail("schemaVersion must be 1");
}

if (typeof matrix.lastUpdated !== "string" || matrix.lastUpdated.length === 0) {
    fail("lastUpdated must be a non-empty string");
}

if (!Array.isArray(matrix.parityStates)) {
    fail("parityStates must be an array");
}

if (!Array.isArray(matrix.capabilities)) {
    fail("capabilities must be an array");
}

if (!Array.isArray(matrix.outOfScopeCommands)) {
    fail("outOfScopeCommands must be an array");
}

const idsSeen = new Set();
for (const capability of matrix.capabilities) {
    if (!capability || typeof capability !== "object") {
        fail("each capabilities entry must be an object");
    }

    if (typeof capability.id !== "string" || capability.id.length === 0) {
        fail("capability.id must be a non-empty string");
    }

    if (idsSeen.has(capability.id)) {
        fail(`duplicate capability id: ${capability.id}`);
    }
    idsSeen.add(capability.id);

    if (typeof capability.status !== "string" || !ALLOWED_STATES.has(capability.status)) {
        fail(`capability '${capability.id}' has invalid status '${capability.status}'`);
    }

    if (typeof capability.ticket !== "string" || capability.ticket.length === 0) {
        fail(`capability '${capability.id}' is missing ticket reference`);
    }
}

for (const requiredId of REQUIRED_CAPABILITIES) {
    if (!idsSeen.has(requiredId)) {
        fail(`missing required capability id: ${requiredId}`);
    }
}

const unknownRequired = [...idsSeen].filter((id) => !REQUIRED_CAPABILITIES.includes(id));
if (unknownRequired.length > 0) {
    fail(`unknown capability ids found: ${unknownRequired.join(", ")}`);
}

console.log(`[parity-matrix] OK: ${matrix.capabilities.length} capability rows validated`);
