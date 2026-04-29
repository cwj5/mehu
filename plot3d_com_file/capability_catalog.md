# PLOT3D Capability Catalog

Last updated: 2026-04-03

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
   - Select function by legacy number, translated to canonical typed model across all ranges:
     - 0–99: grid-diagnostic / geometry modes (`GridFunction` — walls, grids, IBLANK holes, orphan points, crossing/tet checks).
     - 100–199: scalar field selection (`ScalarField` — all 48+ variants fully implemented).
     - 200–299: vector field selection (`VectorField` — velocity, vorticity, momentum, perturbation velocity, V×ω, pressure/density gradients; rendering deferred).
     - 300–399: particle/stream-trace selection (`ParticleFunction` — particle traces, vortex lines; rendering deferred).
     - 400+: special overlay selection (`SpecialFunction` — shock by pressure gradient, filtered variant; rendering deferred).
   - All recognized IDs produce deterministic `PlotState` mutations and diagnostics; unknown IDs within each range produce warnings.
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
   - Configure bounded-MVP function-surface behavior (iso-level + FUNCTION scalar field).
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
