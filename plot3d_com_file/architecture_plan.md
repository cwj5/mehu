# PLOT3D .com File Support: Architecture Plan

Last updated: 2026-03-11

## Purpose

Implement legacy PLOT3D `.com` command-file support in `overview` without creating a separate scripting-only path. Script execution and GUI interaction must converge on the same internal model so supported script features also exist in the GUI.

This document is the canonical handoff for another agent or engineer picking up the work.

## Primary Goals

1. Read legacy `.com` files and apply supported commands.
2. Update the existing GUI/view panel from command execution.
3. Preserve script/GUI parity for supported capabilities.
4. Support PNG export from command files.
5. Build the architecture so deterministic export and interactive rendering share the same logical plot state and render intent.

## Non-Goals

1. Full legacy utility-command coverage is not required.
2. Exact visual reproduction of legacy PLOT3D output is not required.
3. Bitwise-identical PNGs across machines are ideal but not required.
4. Generating legacy `.com` files from the GUI is not a goal.
5. Adding new legacy PLOT3D commands is not a goal.

## Success Criteria

1. Supported command files produce the expected `PlotState`.
2. Equivalent GUI interactions produce the same `PlotState` and `RenderIntent`.
3. `PLOT` commands generate deterministic render intents.
4. In-app export and headless export are visually equivalent for the same render intent.
5. Supported script features are not left script-only in released scope.

## Locked Decisions

1. Incremental delivery is acceptable, but each release must publish a parity matrix.
2. A single shared `PlotState` is the source of truth. No dual script state and GUI state machines.
3. Parity target is equality of `PlotState` and `RenderIntent`, not pixel-identical rendering.
4. Unsupported commands soft-fail: emit a warning, ignore the command, continue execution.
5. Script-only knobs are temporarily allowed, but they must be tracked in the parity matrix and must not be marked fully supported.
6. Legacy `.com` files are read-only input. GUI save/export may eventually use a different format, not legacy `.com`.
7. The canonical semantic model for legacy `FUNCTION` values remains the existing `ScalarField` enum. The parser/executor will translate legacy function numbers into that enum.
8. Contour values must use absolute physical values, not normalized `0..1` values.
9. Multi-grid contour scaling should use the global field range across all loaded grids.
10. Multi-level contours must be supported. The contour model cannot be limited to one level.
11. GUI interactions that feel continuous should use ephemeral frontend state during drag and only commit to shared `PlotState` on apply or at least on control release.
12. Shared Rust render-intent-to-export logic is strongly preferred for GUI export and CLI export. Separate implementations are allowed only if one shared path is not practical.
13. Include-file path handling is currently expected to be relative to the file being parsed, but this still needs confirmation against legacy behavior.
14. `READ` file path resolution is still open and should be verified during implementation.
15. A translation layer between legacy PLOT3D conventions and Three.js conventions is expected. Exact legacy camera/axis behavior is not required, but the translation must be deterministic and documented.
16. Existing index-slice behavior should evolve toward `SUBSETS`/`WALLS` semantics instead of remaining a separate simplified model.
17. TDD is preferred for parity-critical parts: parser, transitions, and equivalence tests should be written before or alongside implementation.
18. Architecture Decision Records are required for at least commit semantics, unsupported-command policy, contour value model, coordinate translation, export determinism policy, and render path choice.

## Current Codebase Context

### Frontend

1. `src/App.tsx`
   - Holds most UI state today.
   - Currently stores contour level as normalized `0..1` (`contourLevel`), which conflicts with the new absolute-value requirement.
   - Manages slices, scalar field selection, color scheme, wireframe, arbitrary planes, and contour mode.

2. `src/components/Viewer3D.tsx`
   - Performs mesh generation by invoking Tauri commands.
   - Already supports grid slices, arbitrary plane slices, solution-colored geometry, iso-surfaces, and contour lines.
   - Currently assumes a GUI-driven state model rather than a shared backend `PlotState`.

3. `src/types/plot3d.ts`
   - Holds TypeScript interfaces for cached metadata and contour-related structures.

4. `src/utils/solutionData.ts`
   - Defines the frontend `ScalarField` enum/type used by the app.
   - This enum should remain canonical; legacy function numbers should map into it.

### Backend

1. `src-tauri/src/lib.rs`
   - Exposes Tauri commands for cached loading, slicing, coloring, contour extraction, iso-surface extraction, logging, dialogs, etc.
   - Will be the main integration point for parse/apply/execute/export commands.

2. `src-tauri/src/plot3d.rs`
   - Contains the heavy numerical logic for mesh creation, slices, contours, iso-surfaces, and file loading.
   - Should be reused, not duplicated.

3. `src-tauri/src/solution.rs`
   - Contains scalar field calculations and should anchor legacy `FUNCTION` number semantics.

4. `src-tauri/Cargo.toml`
   - Will need parser/export dependencies and later CLI-export dependencies.

### Existing Useful Tauri Commands

These already exist and should be reused where possible instead of reimplementing behavior:

1. `load_plot3d_file_cached`
2. `load_plot3d_solution_cached`
3. `slice_grid_by_id`
4. `slice_arbitrary_plane_by_id`
5. `compute_solution_colors`
6. `compute_solution_colors_sliced`
7. `compute_solution_colors_arbitrary_plane`
8. `get_solution_field_range`
9. `extract_iso_surface_by_id`
10. `extract_slice_contours_by_id`
11. `extract_arbitrary_plane_contours_by_id`

## Supported Scope Target

The intended visualization-critical capability set is:

1. `READ`
2. `FUNCTION`
3. `VIEW`
4. `VPOINT`
5. `MINMAX`
6. `CONTOURS`
7. `PLOT`
8. `WALLS`
9. `SUBSETS`
10. `FSURFACE`
11. `TEXT`
12. `SHOW`

Explicitly out of current target scope:

1. `HELP`
2. `LIST`
3. `MAP`
4. `CLEAR`
5. Full journal/QUIT/EXIT save semantics
6. `VECTORS`
7. `RAKES`

## Architectural Direction

### 1. Shared Domain Model

Introduce a backend-owned `PlotState` that holds all plot configuration independent of whether it came from script parsing or GUI interactions.

Expected responsibilities:

1. Active dataset/grid/solution references
2. Current scalar function selection
3. Contour specification
4. View/camera/up-axis choices
5. MINMAX ranges
6. FSURFACE configuration
7. WALLS and SUBSETS definitions
8. Plot text
9. Plot mode (`2D`/`3D`, contour/surface/line, axes/background/options)

### 2. Shared Action API

Both parser execution and GUI interactions should produce typed `PlotAction` values.

Example direction:

1. `SetFunction(ScalarField)`
2. `SetView(ViewSpec)`
3. `SetContourSpec(ContourSpec)`
4. `AddSubset(SubsetSpec)`
5. `SetFsurface(FsurfaceSpec)`
6. `SetPlotText(TextSpec)`
7. `CommitPlot(PlotOptions)`

All supported state mutation should flow through `apply_action` or equivalent.

### 3. RenderIntent Boundary

`PLOT` should remain the script commit boundary. Commands before `PLOT` mutate pending state only. When `PLOT` executes, the system should emit a `RenderIntent` derived from the current `PlotState`.

The `RenderIntent` should be the stable bridge between:

1. Interactive GUI rendering
2. In-app export
3. Headless CLI export

### 4. GUI Commit Model

Not every GUI interaction should round-trip into backend state on every frame.

Preferred model:

1. Local transient GUI state during drag/edit
2. Commit to backend `PlotState` on apply/release
3. Re-render based on the resulting shared state

This avoids fragile IPC-heavy interactions while preserving parity at meaningful state boundaries.

### 5. Legacy-to-Modern Translation Layer

The system should explicitly translate legacy command semantics into modern rendering semantics.

This includes:

1. `FUNCTION` number to `ScalarField`
2. Legacy axis/view choices to Three.js camera conventions
3. Legacy contour modes to existing contour and iso-surface extraction paths
4. `SUBSETS`/`WALLS` range semantics to current and future slice-selection models

This layer should be documented because exact legacy rendering parity is not required.

## Important Data Model Requirements

### PlotState

The following requirements are already known:

1. Contours cannot be modeled as a single normalized number.
2. Contours must support multiple entries.
3. Entries should support at least:
   - a single absolute contour level
   - a range triple `(start, end, increment)`
   - an automatic mode if `CONTOURS/AUTOMATIC` is implemented
4. `FUNCTION` storage should use `ScalarField`, not raw legacy integers.
5. Legacy command diagnostics need file/line/column when available.

Suggested contour model:

1. `ContourSpec::Automatic { max_levels }`
2. `ContourSpec::Increment { increment }`
3. `ContourSpec::Manual { entries: Vec<ContourEntry> }`

Suggested contour entry model:

1. `Single(value)`
2. `Range { start, end, increment }`

### FUNCTION Number Mapping

Legacy scripts use integers like `114` for pressure coefficient. The application should keep using the modern scalar-field enum and maintain a lookup table in the parser/execution layer.

Requirements:

1. Known values map deterministically to `ScalarField`.
2. Unknown function numbers emit a warning and are ignored.
3. The mapping should eventually cover all legacy values relevant to supported visualization features.

### SUBSETS and WALLS

These likely need a richer internal model than the current slice-per-grid UI provides.

Requirements:

1. Range-oriented specs per grid
2. Ability to represent first/last/stride selections
3. Clear mapping into current slice and mesh extraction behavior
4. GUI affordances that expose the same conceptual power as the script path

## Testing Strategy

Parity-critical work should follow TDD or near-TDD.

### Required test categories

1. Parser tests
   - command syntax
   - abbreviations
   - qualifiers
   - malformed input
   - include-file handling

2. State-transition tests
   - applying actions yields expected `PlotState`
   - unsupported commands produce warnings, not fatal errors

3. Equivalence tests
   - script path and GUI path produce the same `PlotState`
   - equal `PlotState` produces equal `RenderIntent`

4. Regression tests
   - current manual workflows still behave correctly after refactor

5. Export tests
   - in-app export and CLI export are visually equivalent for the same `RenderIntent`

## Risks and Fragile Areas

### 1. SUBSETS/WALLS vs current slice model

Current slice UI is simpler than legacy selection semantics. This is a likely refactor hotspot.

Mitigation:

1. Treat slice refactor as part of parity work, not as an afterthought.
2. Use an intermediate selection model instead of forcing direct UI-to-geometry mapping.

### 2. Coordinate and camera translation

Legacy `VIEW`, `VPOINT`, and `UP` behavior will not map perfectly to Three.js.

Mitigation:

1. Create an explicit translation layer.
2. Document known differences.
3. Test determinism, not pixel identity.

### 3. Current normalized contour UI

The app currently stores contour level as `0..1`. That is incompatible with script semantics.

Mitigation:

1. Migrate early in the shared-state work.
2. Add regression tests around contour behavior.

### 4. Path resolution behavior

Legacy behavior for `@include` and maybe `READ` relative paths is not confirmed.

Mitigation:

1. Make path resolution policy explicit in code.
2. Verify against legacy behavior when implementation reaches that point.

### 5. Export path divergence

GUI export and CLI export can drift if they use separate rendering logic.

Mitigation:

1. Prefer one shared render/export path.
2. If impossible, write an ADR and parity tests to keep them aligned.

## Recommended Initial Work Order

1. Define capability catalog and parity matrix
2. Define shared `PlotState`, `PlotAction`, and diagnostics types
3. Define the `FUNCTION` number mapping table shape
4. Define the contour model with multi-level support
5. Implement parser and parser tests
6. Implement executor and `RenderIntent`
7. Refactor GUI onto shared actions/state
8. Close GUI parity gaps for `SUBSETS`, `WALLS`, `FSURFACE`, `TEXT`, and `SHOW`
9. Implement in-app export
10. Implement headless CLI export

## Files Likely to Change

1. `src/App.tsx`
2. `src/components/Viewer3D.tsx`
3. `src/types/plot3d.ts`
4. `src/utils/solutionData.ts`
5. `src-tauri/src/lib.rs`
6. `src-tauri/src/plot3d.rs`
7. `src-tauri/src/solution.rs`
8. `src-tauri/Cargo.toml`

## Open Questions Still Deferred

1. Exact legacy path resolution rules for `READ`
2. Exact legacy path resolution rules for `@include`
3. Whether `CONTOURS/AUTOMATIC` needs a fully preserved legacy behavior or only a modern equivalent
4. Whether one shared export implementation is practical enough, or whether GUI export and CLI export need separate implementations

## Reference Documents

1. `plot3d.md`
2. `PLOT3D_COMMANDS.md`
3. `CONTOUR_LEVELS_IMPLEMENTATION.md`
4. `ARBITRARY_PLANES.md`
5. `IBLANK_FILTERING_IMPLEMENTATION.md`
