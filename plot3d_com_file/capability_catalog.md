# PLOT3D Capability Catalog

Last updated: 2026-03-12

## Purpose

This catalog defines the canonical capability IDs for script/GUI parity tracking.
Every release must publish parity status for these capabilities in `plot3d_com_file/parity_matrix.json`.

## Parity States

Allowed status values:

1. `supported`
2. `script-only`
3. `gui-only`
4. `not-supported`

## In-Scope Visualization Capabilities

1. `READ`
   - Load grid/solution/function data from command context.
2. `FUNCTION`
   - Select scalar/vector function by legacy function number (translated to canonical model).
3. `VIEW`
   - Select axis pairing / view orientation.
4. `VPOINT`
   - Set viewpoint in Cartesian or angular form.
5. `MINMAX`
   - Set axis range constraints and optional increments.
6. `CONTOURS`
   - Configure contour generation (automatic/increment/manual) using absolute values.
7. `PLOT`
   - Commit current state to a render intent.
8. `WALLS`
   - Define wall-selection regions and related attributes.
9. `SUBSETS`
   - Define active subset regions and related attributes.
10. `FSURFACE`
    - Configure function-surface behavior (scale/origin/mode).
11. `TEXT`
    - Configure plot text lines.
12. `SHOW`
    - Display structured current state and command status.

## Out-of-Scope Legacy Commands (Current Phase)

1. `HELP`
2. `LIST`
3. `MAP`
4. `CLEAR`
5. `EXIT` journal/save semantics
6. `QUIT` journal/save semantics
7. `VECTORS`
8. `RAKES`

## Inclusion Rule

A capability is considered `supported` only if:

1. It is executable from command files.
2. It has equivalent GUI affordance/behavior for parity scope.
3. It is represented in shared `PlotState` and produces deterministic `RenderIntent` behavior where applicable.
