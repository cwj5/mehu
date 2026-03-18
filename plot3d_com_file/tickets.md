# PLOT3D .com File Support: Tickets

Last updated: 2026-03-17

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

### ~~TKT-003: Define and implement FUNCTION-number mapping~~ ✅ COMPLETE

Completed: 2026-03-12. Added deterministic legacy FUNCTION mapping in `src-tauri/src/function_mapping.rs`, with tests for supported values, known-unimplemented soft-fail behavior, and unknown/out-of-scope warnings. Canonical `ScalarField` now includes placeholder variants for known legacy scalar functions and explicit `TODO_EQUATION` markers where formulas are not yet implemented.

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

### ~~TKT-004: Implement strict parser and validator for `.com` files~~ ✅ COMPLETE

Completed: 2026-03-12. Implemented `src-tauri/src/com_parser.rs` with tokenizer, alias table, include handling (with cycle detection), command dispatch for all in-scope capabilities, and 15 unit tests covering all three acceptance criteria.

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

### ~~TKT-005: Implement command executor and RenderIntent~~ ✅ COMPLETE

Completed: 2026-03-12. Added `src-tauri/src/script_executor.rs` with `RenderIntent`, `ScriptExecutionResult`, `execute_actions`, and `execute_parsed_script`; wired `execute_com_script` Tauri command in `src-tauri/src/lib.rs`; `PLOT` now emits render intents at commit boundaries, `SHOW` produces executor output, parsed and execution diagnostics are merged, and final `PlotState` is persisted to shared backend state.

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

### ~~TKT-006: Refactor GUI state flow to use PlotAction commits~~ ✅ COMPLETE

Started: 2026-03-12. First migration slice complete: scalar-field and contour controls in `src/App.tsx` now dispatch backend `apply_plot_action` updates, `SolutionViewer` is controlled from app-level state, and a backend `PlotState` dev inspector panel was added to the sidebar.

Update: 2026-03-15. Camera view presets now commit through shared plot-state actions (`SetAxisView` via `set_plot_axis_view`) and `PLOT` commit boundaries from the GUI, keeping `VIEW`/`VPOINT` camera interactions on the same backend state path used by script execution. Fixed serde mismatch where `rename_all = "snake_case"` produced `"plane_x_y"` instead of `"plane_xy"` for plane view variants; explicit `#[serde(rename)]` overrides added and covered by a Rust round-trip test.

Update: 2026-03-17. Subset/slicing apply flow replaced: the old implicit `useEffect`-driven per-edit push was removed in favour of an explicit "Apply Slicing to PlotState" button (and Enter-key alias) that calls `set_plot_subsets` → `commit_plot` as a single atomic boundary. `CameraCommitControls` in `Viewer3D.tsx` now ignores programmatic camera moves so only user drag events schedule a commit. Frontend integration test suite added (`App.integration.test.tsx`, 4 tests) covering axis-view wiring, plane_xy serde regression, contour mode, and Enter-to-apply slices. Rendering-hint state (`ignoreIblank`, `showWireframe`, `shadingMode`) confirmed absent from the parity matrix and explicitly left as local React state — not in scope.

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

### TKT-007 (Epic): Absolute multi-level contours and plot-family semantic alignment

Updated scope: 2026-03-17. This is now an umbrella ticket split into implementation-sized parts (TKT-007A through TKT-007E).

Execution metadata:

1. Estimated effort: XL
2. Primary owner role: Feature lead coordinating backend + frontend
3. Suggested contributors: parser/backend owner, frontend/viewer owner, test owner
4. Exit condition: all TKT-007A through TKT-007E acceptance criteria satisfied

Goal:
Replace the normalized single-level contour path with an absolute-valued, multi-level contour system and align contour versus function-surface or carpet semantics with legacy behavior.

Why this exists:
The old path conflates parity concepts and local viewer toggles, still uses normalized contour assumptions in key places, and blurs contour-plot behavior with function-surface behavior.

Implementation context:

1. Contour plots represent scalar values by level location.
2. Function-surface or carpet plots represent scalar magnitude on a plotted function axis against one or two spatial axes.
3. `CONTOURS` sets contour levels and contour attributes; it does not choose the plot family.
4. Plot-family selection must not be encoded through the retired `Enable Contours` checkbox and `surfaces / lines / both` display shortcut.

Epic-level acceptance criteria:

1. No normalized contour-value contract remains across shared state, GUI contracts, or extraction IPC.
2. `AUTOMATIC`, `INCREMENT`, and `MANUAL` contour specs round-trip script and GUI flows without truncation.
3. Contour plots and function-surface or carpet plots follow distinct deterministic render paths.
4. First-pass contour attributes exist as shared parity state with explicit behavior and diagnostics for unsupported combinations.

Dependencies:

1. TKT-002
2. TKT-005
3. TKT-006

Relevant decisions:

1. No normalized contour values.
2. Multi-level contour support is mandatory.
3. Default contour attribute is `LINE`.
4. `COLOR CONTOURS` first pass uses the existing field colormap on filled geometry.
5. Function-surface or carpet rendering may ship as a bounded MVP if incomplete paths are explicit.
6. Legacy contour attributes are shared parity state; wireframe and shading remain local viewer styling.

### TKT-007A: Shared state and parser contracts for contour specs, attributes, and plot families

Execution metadata:

1. Estimated effort: M
2. Primary owner role: Backend/parser engineer
3. Secondary owner role: Integration reviewer for script compatibility

Goal:
Finalize backend state and parser contracts so contour levels, contour attributes, and plot-family choices are represented unambiguously in shared state.

Scope:

1. Refine shared `PlotState` and `PlotAction` representations for contour levels and first-pass contour attributes: `LINE`, `SURFACE`, `GRID`, `COLOR CONTOURS`, and `DOTS`.
2. Ensure plot-family representation is explicit and does not rely on old UI shortcut semantics.
3. Update parser handling for `CONTOURS`, `PLOT`, and related qualifiers to populate the refined shared model.
4. Stop silently ignoring contour-attribute intent in parser/state transitions.

Acceptance criteria:

1. Shared state can express all three contour-spec modes and first-pass contour attributes.
2. Parser emits deterministic actions for `CONTOURS` level modes and plot-family switches.
3. Unknown or unsupported qualifiers generate diagnostics rather than silent behavior changes.

Dependencies:

1. TKT-002
2. TKT-004
3. TKT-006

### TKT-007B: Absolute contour-level resolution and IPC contract migration

Execution metadata:

1. Estimated effort: M
2. Primary owner role: Backend/IPC engineer
3. Secondary owner role: Viewer integration engineer

Goal:
Remove normalized contour-value contracts and standardize one absolute contour-level resolution path.

Scope:

1. Replace normalized contour extraction arguments with absolute level inputs across Tauri commands.
2. Add a backend helper that resolves `AUTOMATIC`, `INCREMENT`, and `MANUAL` specs into explicit absolute levels using field range context.
3. Ensure degenerate range handling is deterministic and diagnostic-friendly.

Acceptance criteria:

1. No extraction command requires a normalized contour input.
2. All contour-level resolution flows use one canonical absolute-level resolver.
3. Uniform-field cases avoid divide-by-zero behavior and emit deterministic outcomes.

Dependencies:

1. TKT-007A

### TKT-007C: GUI contour model and editor migration

Execution metadata:

1. Estimated effort: L
2. Primary owner role: Frontend app engineer
3. Secondary owner role: Backend reviewer for state/action shape alignment

Goal:
Replace the old single-level contour UI with a parity-aligned plot-family and contour editor flow.

Scope:

1. Retire `Enable Contours` and the `surfaces / lines / both` control.
2. Add GUI controls for plot-family selection and contour-spec modes: `AUTOMATIC`, `INCREMENT`, `MANUAL`.
3. Add first-pass GUI controls for contour attributes with default `LINE` behavior.
4. Ensure backend-to-UI sync round-trips script-driven contour specs without collapsing to one manual level.

Acceptance criteria:

1. GUI can author and edit all three contour-spec modes.
2. Script-loaded contour specs and attributes remain intact after GUI sync and apply.
3. Old single-level behavior exists only as a subset of manual mode.

Dependencies:

1. TKT-007A
2. TKT-007B

### TKT-007D: Renderer semantic split and bounded function-surface MVP

Execution metadata:

1. Estimated effort: L
2. Primary owner role: Frontend viewer/render engineer
3. Secondary owner role: Backend reviewer for render-intent/contract consistency

Goal:
Implement deterministic renderer-path separation for contour plots versus function-surface or carpet plots.

Scope:

1. Split contour rendering from function-surface or carpet rendering in viewer integration paths.
2. Remove old display-mode mapping assumptions from rendering code.
3. Implement first-pass contour-attribute behavior with explicit diagnostics for unsupported combinations.
4. Ship a bounded function-surface MVP where complete parity is not yet available.

Acceptance criteria:

1. Contour and function-surface or carpet paths are distinct and deterministic.
2. First-pass `COLOR CONTOURS` uses field colormap on filled geometry.
3. Unsupported combinations surface diagnostics or explicit UI messaging, not silent fallback.

Dependencies:

1. TKT-007A
2. TKT-007B
3. TKT-007C

### TKT-007E: TKT-007 parity and regression test coverage

Execution metadata:

1. Estimated effort: M
2. Primary owner role: Test/integration engineer
3. Secondary owner role: backend + frontend code owners for review

Goal:
Lock in the TKT-007 behavior with targeted backend and frontend coverage.

Scope:

1. Add unit tests for contour spec parsing/state transitions and contour-level resolution edge cases.
2. Add integration tests for GUI plot-family and contour-spec commits.
3. Add contract tests that verify absolute-level arguments reach extraction commands.
4. Add regression tests for unsupported-combination diagnostics.

Acceptance criteria:

1. New test suite fails on normalized contour regressions.
2. GUI tests cover Automatic, Increment, Manual, and plot-family round-trips.
3. Diagnostics behavior for unsupported combinations is test-covered.

Dependencies:

1. TKT-007A
2. TKT-007B
3. TKT-007C
4. TKT-007D

### TKT-007 Implementation Reference (future handoff)

Purpose:
Capture concrete implementation anchors and non-negotiable decisions so future agents can execute without rediscovery.

Key non-negotiable decisions:

1. Contour values are absolute physical values end-to-end; normalized contour-value contracts are forbidden.
2. `CONTOURS` is a level-and-attribute command. It does not choose plot family.
3. Plot-family semantics must not reuse the retired `Enable Contours` and `surfaces / lines / both` shortcut.
4. Default contour attribute is `LINE`.
5. First-pass `COLOR CONTOURS` uses field colormap on filled geometry (no per-level GUI color editor required in this ticket).
6. Unsupported combinations must emit diagnostics or explicit UI messaging; no silent fallback.
7. Wireframe/shading stay local viewer styling for TKT-007.

Code touchpoints to expect for TKT-007 work:

1. `src-tauri/src/plot_state.rs`: canonical contour spec/attribute/plot-family state and `apply_action` behavior.
2. `src-tauri/src/com_parser.rs`: `CONTOURS`, `PLOT`, and related qualifier parsing.
3. `src-tauri/src/lib.rs`: Tauri contour extraction commands and IPC argument contracts.
4. `src-tauri/src/script_executor.rs`: render-intent boundary behavior at `PLOT` commits.
5. `src/types/plot3d.ts`: frontend type contract mirrors for contour/plot-family state.
6. `src/App.tsx`: contour editor controls, plot-family controls, and backend-state sync.
7. `src/components/Viewer3D.tsx`: contour/function-surface rendering path split and extraction call arguments.
8. `src/App.integration.test.tsx`: GUI commit flow and regression tests.

Source references for legacy semantics:

1. `plot3d.md`:
   - `CONTOURS` modes and qualifier behavior.
   - `FSURFACE` semantics and relationship to `VIEW`, `MINMAX`, and contour attributes.
   - `PLOT` qualifiers (`/CONTOUR`, `/SURFACE`, `/CARPET`, `/LINE`) and expected behavior distinctions.
2. `plot3d_com_file/capability_catalog.md`: canonical in-scope capability definitions.
3. `plot3d_com_file/parity_matrix.json`: current parity status and ticket ownership.
4. The scalar-function possibility chart (project discussion artifact): interpret as behavioral guidance for first-pass attribute semantics (`LINE`, `SURFACE`, `GRID`, `COLOR CONTOURS`, `DOTS`).

Contract migration checklist:

1. Remove any remaining `levelNormalized` contour arguments at frontend/backend boundaries.
2. Ensure one canonical resolver computes explicit contour levels for `AUTOMATIC`, `INCREMENT`, and `MANUAL` specs.
3. Verify manual absolute values never get silently reinterpreted as normalized fallback values.
4. Verify degenerate-range behavior is deterministic and test-covered.

Recommended completion evidence for TKT-007 sub-tickets:

1. Backend tests: contour-state transitions, parser behavior, and level-resolution edge cases.
2. Frontend tests: plot-family + Automatic/Increment/Manual editor round-trips and commit ordering.
3. Contract tests: extraction commands receive absolute levels only.
4. Manual scenario checks:
   - `CONTOURS 5` + `PLOT/CONTOUR` reflects Automatic mode in GUI.
   - Manual multi-level entries persist through apply/reload cycles.
   - Function-surface path is distinct from contour path.

Current verification baseline (as of 2026-03-17):

1. `cargo test plot_state` passes in `src-tauri`.
2. `npm run test -- src/App.integration.test.tsx` passes in repo root.

### TKT-008: Close GUI gaps for WALLS, SUBSETS, FSURFACE, TEXT, and SHOW

Goal:
Add or refactor GUI affordances so supported script features are also available interactively.

Why this exists:
Feature parity is an explicit goal.

Scope:

1. Design a richer range-based selection model for `SUBSETS` and `WALLS`.
2. Update or replace current index-slice controls so they align with that model.
3. Add `FSURFACE` controls beyond the bounded MVP separation delivered in TKT-007, including scale factor, walls origin, and any needed mode controls.
4. Add plot-text controls.
5. Add a `SHOW` status view.

Acceptance criteria:

1. No supported feature remains script-only.
2. Index slices are aligned with subset/wall semantics or replaced by a better model.
3. The parity matrix reflects GUI support accurately.

Dependencies:

1. TKT-006
2. TKT-007D

Relevant decisions:

1. Existing slice behavior should evolve toward subset/wall semantics.
2. Script-only knobs can exist temporarily but must be tracked.
3. TKT-007 is responsible for separating contour plots from function-surface or carpet plots; TKT-008 finishes the missing interactive `FSURFACE` controls.

### TKT-009: Implement deterministic legacy-to-Three.js translation layer

Goal:
Formalize how legacy plotting concepts translate to the modern rendering stack.

Why this exists:
Without a formal translation layer, render behavior will be brittle and undocumented.

Scope:

1. Map `VIEW` and `VPOINT` into camera behavior.
2. Map `PLOT/UP` and related orientation options.
3. Define how contour, function-surface, carpet, and line plot families use current geometry-generation paths, building on the semantic split introduced in TKT-007.
4. Document known differences from legacy output.

Acceptance criteria:

1. Translation is deterministic.
2. Known deviations are documented.
3. Equal `RenderIntent` yields predictable interactive and export results.

Dependencies:

1. TKT-005
2. TKT-007D

Relevant decisions:

1. Exact visual legacy parity is not required.
2. Documented deterministic behavior is required.
3. TKT-007 may ship a bounded function-surface MVP, but TKT-009 is where the long-term translation rules and documented deviations must be finalized.

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
2. Milestone B: TKT-006, TKT-007A through TKT-007E, TKT-008, TKT-009
3. Milestone C: TKT-010 through TKT-011
4. Milestone D: TKT-012

## Parallelism Notes

1. TKT-003 can overlap with TKT-002 once state types are stable enough.
2. TKT-007A should land before TKT-007B, TKT-007C, and TKT-007D begin full implementation.
3. TKT-007B and TKT-007C can overlap once shared contracts from TKT-007A are stable.
4. TKT-007D should start after TKT-007B contract migration is in place and TKT-007C has established new GUI control semantics.
5. TKT-007E runs continuously but should not be considered complete until TKT-007D behavior is merged.
6. TKT-010 and TKT-011 should share as much render/export logic as practical.

## Things Another Agent Should Not Re-Decide

1. Do not reintroduce normalized contour values.
2. Do not make raw legacy function integers the canonical internal function representation.
3. Do not make script execution a special path that bypasses GUI/state architecture.
4. Do not silently skip unsupported commands without diagnostics.
5. Do not assume exact legacy visual parity is required.
6. Do not design continuous GUI editing to write through IPC on every drag event.
7. Do not treat the old `surfaces / lines / both` contour control as a parity concept.
8. Do not collapse contour plots and function-surface or carpet plots back into one ambiguous rendering mode.
9. Do not promote current wireframe or shading viewer toggles into shared parity state as part of TKT-007.
10. Do not silently fall back from unsupported contour-attribute or plot-family combinations.
