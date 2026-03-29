# Safest Path Execution Plan

Last updated: 2026-03-28

## Objective

Execute a hardening-first path before any new feature expansion.

This plan prioritizes:

1. Truthful parity status and governance alignment.
2. Lower regression risk across parser/executor and cross-path parity.
3. Stronger headless export regression confidence.

Feature work resumes only after all hardening gates pass in a single stability run.

## Scope

Included in this cycle:

1. Parity matrix and parity-facing documentation reconciliation.
2. Backend and cross-path parity hardening.
3. Headless export regression hardening (deterministic and semantic checks).
4. Stability sign-off and release-readiness gating.

Explicitly out of scope until sign-off:

1. New user-facing features (vector field visualization, point probe, LOD expansion, full 74-function implementation).

## Phase Plan

### Phase A (Days 1-3): Parity Truth Refresh

1. Audit all 12 capability entries in `plot3d_com_file/parity_matrix.json` against delivered behavior and completed tickets.
2. Update status, notes, and rationale fields for each capability.
3. Sync parity-facing docs and runbooks:
   - `README.md`
   - `TESTING.md`
4. Run local parity governance checks and resolve every mismatch before proceeding.

### Phase B (Week 1): Backend and Cross-Path Hardening

1. Expand Rust test coverage in parser/executor risk areas:
   - malformed command diagnostics
   - include resolution edge cases
   - action transition edge paths
   - commit-boundary invariants
2. Strengthen script-vs-GUI parity integration checks for high-risk state transitions:
   - camera/view and up-vector behavior
   - contours and contour-level behavior
   - subsets/walls behavior
   - commit boundaries for plot application

### Phase C (Week 2): Headless Regression Hardening

1. Keep deterministic baseline fixtures and hash checks.
2. Add semantic image checks with tolerance-based thresholds (for meaningful drift detection).
3. Document divergence policy and failure-triage steps for faster debugging.

### Phase D (Exit Gate): Stability Sign-Off

All gates must pass together in one run before feature work resumes.

### Phase D Sign-Off (2026-03-28)

**Status:** All required validation gates have passed in a single run:

- Parity matrix validation (`npm run validate:parity-matrix`): Passed
- Backend parity fixture (`npm run test:parity-backend`): Passed
- Cross-path parity integration (`npm run test:parity-cross-path`): Passed
- Headless regression (hash + semantic) (`headless-export/fixtures/regression/check_regression.sh`): Passed
- Full TypeScript/JS test suite (`npm test`): Passed
- Rust backend library tests (`cd src-tauri && cargo test --lib`): Passed

No errors or failures detected. Codebase is stable and ready for feature-unfreeze.

**Sign-off:** Stability exit gate achieved. Feature work may now resume per roadmap.

## Required Validation Gates

Run from repository root unless noted.

1. `npm run validate:parity-matrix`
2. `npm run test:parity-backend`
3. `npm run test:parity-cross-path`
4. `headless-export/fixtures/regression/check_regression.sh`
5. `npm run test`
6. `cd src-tauri && cargo test --lib`

## Deliverables

Phase A deliverables:

1. Updated parity matrix with accurate capability statuses.
2. Updated parity guidance in README/testing docs.

Phase B deliverables:

1. New/expanded backend parser-executor unit tests.
2. Expanded cross-path parity integration coverage.

Phase C deliverables:

1. Semantic regression checks for headless export.
2. Updated troubleshooting and divergence documentation.

Phase D deliverables:

1. Single green stability run across all required gates.
2. Hardening sign-off note confirming feature-unfreeze readiness.

## Blockers and Decision Rules

1. If parity audit uncovers real behavior gaps, fix those gaps before continuing hardening.
2. If CI runtime becomes problematic, optimize execution order and fixtures, but do not weaken gate semantics.
3. If any gate fails during sign-off, no new feature scope is opened.

## Next Milestone After Sign-Off

Recommended next milestone after this plan completes:

1. Roadmap Phase 3.1: Built-in function catalog and implementation work.
