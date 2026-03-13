# PLOT3D .com File Support: Tickets

Last updated: 2026-03-12

This ticket set breaks the work into milestones and implementation-sized tasks. It is written so another agent can pick up the work with minimal additional context.

## Milestone A: Foundation

### ~~TKT-001: Define capability catalog and parity matrix~~ ✅ COMPLETE

Completed: 2026-03-12. Artifacts: `plot3d_com_file/capability_catalog.md`, `plot3d_com_file/parity_matrix.json`, `scripts/validate-parity-matrix.mjs`, `.github/workflows/parity-matrix.yml`.

Goal:
Create the canonical list of supported visualization-critical capabilities and establish the parity-matrix artifact that every release must publish.

Why this exists:
Without a capability catalog, script/GUI parity will drift and scope will expand informally.

Scope:

1. Define capability IDs for:
   - `READ`
   - `FUNCTION`
   - `VIEW`
   - `VPOINT`
   - `MINMAX`
   - `CONTOURS`
   - `PLOT`
   - `WALLS`
   - `SUBSETS`
   - `FSURFACE`
   - `TEXT`
   - `SHOW`
2. Define parity states:
   - `supported`
   - `script-only`
   - `gui-only`
   - `not-supported`
3. Define which legacy commands are intentionally out of scope.
4. Add the parity matrix as a repo artifact.
5. Add a lightweight CI check for presence/format of the matrix.

Acceptance criteria:

1. All in-scope capabilities have IDs and parity status.
2. The parity matrix file is checked into the repo.
3. Out-of-scope commands are explicitly listed.

Dependencies:

1. None

Relevant decisions:

1. Incremental delivery is allowed.
2. Supported scope may not contain undeclared script-only features.

### ~~TKT-002: Introduce shared PlotState, PlotAction, and diagnostics model~~ ✅ COMPLETE

Completed: 2026-03-12. Includes `PlotState`, `PlotAction`, diagnostics model, `apply_action`, `get_plot_state` / `apply_plot_action`, absolute multi-level contours, canonical `plot_state::ScalarField`, and `PlotMode` state/action coverage with unit tests.

Goal:
Create the backend-owned canonical state and transition model used by both script execution and GUI interaction.

Why this exists:
This is the core anti-fragility decision. Everything else depends on it.

Scope:

1. Add `PlotState` in Rust.
2. Add `PlotAction` enum in Rust.
3. Add diagnostics type with capability ID, severity, file, line, column, and message.
4. Add `apply_action` transition function.
5. Add `get_plot_state` command for debugging and parity inspection.
6. Define contour data model with absolute values and multi-level support.
7. Define the modern representation of `WALLS`, `SUBSETS`, `VIEW`, `VPOINT`, `MINMAX`, `FSURFACE`, `TEXT`, and plot mode.

Acceptance criteria:

1. Shared state can represent all in-scope capabilities.
2. Contours are absolute-valued, not normalized.
3. Multi-level contour representation exists.
4. The state transition API is covered by unit tests.

Dependencies:

1. TKT-001

Relevant decisions:

1. Shared `PlotState` is the only source of truth.
2. GUI writes to it only on apply/release.
3. `ScalarField` stays canonical.

### TKT-003: Define and implement FUNCTION-number mapping

Goal:
Create the mapping from legacy `FUNCTION` integers to the existing frontend/backend scalar-field model.

Why this exists:
The parser should not leak raw legacy function numbers deep into the app.

Scope:

1. Enumerate supported legacy scalar-function numbers.
2. Map them to `ScalarField` variants.
3. Document unsupported or unknown values.
4. Add tests for known and unknown mappings.

Acceptance criteria:

1. Known values map deterministically.
2. Unknown values warn and soft-fail.
3. The mapping is reusable by parser, executor, and GUI labels if needed.

Dependencies:

1. TKT-002

Relevant decisions:

1. The enum remains canonical.
2. Legacy number parsing is a translation layer concern.

### TKT-004: Implement strict parser and validator for `.com` files

Goal:
Parse supported legacy command syntax into typed actions.

Why this exists:
No parser exists today.

Scope:

1. Tokenize command words, qualifiers, tuples, quoted text, comments, and blank prompt-style input.
2. Parse supported commands into `PlotAction` values.
3. Add abbreviation alias table.
4. Implement include-file handling.
5. Emit warnings for unsupported commands and unknown qualifiers.

Acceptance criteria:

1. Example command files from `plot3d.md` parse successfully for supported commands.
2. Unsupported commands emit warnings and continue.
3. Malformed inputs produce useful diagnostics.

Dependencies:

1. TKT-002
2. TKT-003

Relevant decisions:

1. Soft-fail unsupported commands.
2. Include-file paths are expected to be relative to the parsed file until proven otherwise.

### TKT-005: Implement command executor and RenderIntent

Goal:
Apply parsed actions to `PlotState` and emit `RenderIntent` values on `PLOT`.

Why this exists:
Parsing alone does not update the view panel or export anything.

Scope:

1. Execute actions in order.
2. Mutate pending `PlotState`.
3. Emit `RenderIntent` only when `PLOT` is encountered.
4. Support `SHOW` output and `TEXT` storage.
5. Return execution diagnostics.

Acceptance criteria:

1. `PLOT` is the only render-intent commit boundary.
2. Equal `PlotState` yields equal `RenderIntent`.
3. Execution result contains final state, intents, and diagnostics.

Dependencies:

1. TKT-004

Relevant decisions:

1. Parity target is `PlotState` plus `RenderIntent` equality.
2. Script execution is not allowed to bypass the shared state model.

## Milestone B: GUI parity

### TKT-006: Refactor GUI state flow to use PlotAction commits

Goal:
Move GUI capability-bearing state changes onto the shared action/state path.

Why this exists:
Current GUI logic is mostly local React state, which is incompatible with parity guarantees.

Scope:

1. Audit current `App.tsx` and `Viewer3D.tsx` state changes.
2. Replace direct commits for parity-relevant controls with action dispatches.
3. Keep transient drag/edit state local until apply or release.
4. Add dev inspector for current `PlotState`.

Acceptance criteria:

1. Core capability state is no longer independently owned only by React.
2. Dragging does not flood backend state updates.
3. Dev inspector shows live backend state.

Dependencies:

1. TKT-005

Relevant decisions:

1. Apply/release commit model.
2. Shared state must remain authoritative.

### TKT-007: Replace normalized contour model with absolute multi-level contour model

Goal:
Migrate the current contour implementation away from a single normalized level.

Why this exists:
Legacy command semantics require absolute values and multiple levels.

Scope:

1. Remove `0..1` contour assumptions from frontend and backend integration.
2. Update UI to handle multiple contour entries.
3. Use global field range only for context, not as the stored contour value.
4. Ensure contour extraction paths accept the new model.

Acceptance criteria:

1. Contours are defined in physical values.
2. Multiple contour levels can be configured and displayed.
3. Old single-level GUI becomes a subset of the new model.

Dependencies:

1. TKT-002
2. TKT-005

Relevant decisions:

1. No normalized contour values.
2. Multi-level contour support is mandatory.

### TKT-008: Close GUI gaps for WALLS, SUBSETS, FSURFACE, TEXT, and SHOW

Goal:
Add or refactor GUI affordances so supported script features are also available interactively.

Why this exists:
Feature parity is an explicit goal.

Scope:

1. Design a richer range-based selection model for `SUBSETS` and `WALLS`.
2. Update or replace current index-slice controls so they align with that model.
3. Add `FSURFACE` controls.
4. Add plot-text controls.
5. Add a `SHOW` status view.

Acceptance criteria:

1. No supported feature remains script-only.
2. Index slices are aligned with subset/wall semantics or replaced by a better model.
3. The parity matrix reflects GUI support accurately.

Dependencies:

1. TKT-006
2. TKT-007

Relevant decisions:

1. Existing slice behavior should evolve toward subset/wall semantics.
2. Script-only knobs can exist temporarily but must be tracked.

### TKT-009: Implement deterministic legacy-to-Three.js translation layer

Goal:
Formalize how legacy plotting concepts translate to the modern rendering stack.

Why this exists:
Without a formal translation layer, render behavior will be brittle and undocumented.

Scope:

1. Map `VIEW` and `VPOINT` into camera behavior.
2. Map `PLOT/UP` and related orientation options.
3. Define how contour/surface/line plot modes use current geometry-generation paths.
4. Document known differences from legacy output.

Acceptance criteria:

1. Translation is deterministic.
2. Known deviations are documented.
3. Equal `RenderIntent` yields predictable interactive and export results.

Dependencies:

1. TKT-005
2. TKT-007

Relevant decisions:

1. Exact visual legacy parity is not required.
2. Documented deterministic behavior is required.

## Milestone C: Export

### TKT-010: In-app PNG export from command files

Goal:
Allow users to run a `.com` file and export PNG output from the current app.

Why this exists:
This is the first delivery target for deterministic artifact generation.

Scope:

1. Add command-file run and export workflow to the app.
2. Support one or more `PLOT` outputs per command file.
3. Surface command warnings/errors during export.
4. Reuse shared `RenderIntent` logic.

Acceptance criteria:

1. A user can run a `.com` file and export PNGs from within the GUI.
2. Multi-`PLOT` scripts produce multiple outputs.
3. Warnings remain non-fatal unless the file cannot produce any render intent.

Dependencies:

1. TKT-009

Relevant decisions:

1. Visual equivalence is sufficient.
2. Shared render/export logic is preferred.

### TKT-011: Headless CLI PNG export

Goal:
Create a standalone export path for command files without launching the full GUI.

Why this exists:
Long-term deterministic export should be scriptable and automation-friendly.

Scope:

1. Add a CLI binary target.
2. Reuse parser, executor, and render-intent pipeline.
3. Implement PNG export in a portable way.
4. Keep GUI and CLI behavior aligned through shared logic where practical.

Acceptance criteria:

1. `overview-export --cmd file.com --out out.png` works.
2. Output is visually equivalent to the same render intent in-app.
3. Any divergence from in-app export is documented.

Dependencies:

1. TKT-010 or at minimum the shared render/export core

Relevant decisions:

1. One shared render path is preferred but not mandatory if impractical.
2. Bitwise-identical PNGs are optional.

## Milestone D: Hardening and governance

### TKT-012: Add parity tests, CI gates, and ADRs

Goal:
Prevent long-term drift between script execution and GUI behavior.

Why this exists:
This project will be fragile without automated parity checks and recorded decisions.

Scope:

1. Add parity matrix CI checks.
2. Add parser, state, render-intent, and parity equivalence tests.
3. Add ADRs for key architecture decisions.
4. Update README-level documentation as capabilities become supported.

Acceptance criteria:

1. CI fails when parity or required artifacts regress.
2. ADRs exist for the locked architectural decisions.
3. Another engineer can understand supported scope and known limitations from repo docs.

Dependencies:

1. All prior milestones

Relevant decisions:

1. TDD/prefer-tests-first approach.
2. ADRs are mandatory for fragile architecture choices.

## Recommended Milestone Ordering

1. Milestone A: TKT-001 through TKT-005
2. Milestone B: TKT-006 through TKT-009
3. Milestone C: TKT-010 through TKT-011
4. Milestone D: TKT-012

## Parallelism Notes

1. TKT-003 can overlap with TKT-002 once state types are stable enough.
2. TKT-006 and TKT-007 can overlap after the core action/state system exists.
3. TKT-010 and TKT-011 should share as much render/export logic as practical.

## Things Another Agent Should Not Re-Decide

1. Do not reintroduce normalized contour values.
2. Do not make raw legacy function integers the canonical internal function representation.
3. Do not make script execution a special path that bypasses GUI/state architecture.
4. Do not silently skip unsupported commands without diagnostics.
5. Do not assume exact legacy visual parity is required.
6. Do not design continuous GUI editing to write through IPC on every drag event.
