---
name: "TKT-012C - Cross-Path Parity"
about: "Implementation checklist for script-path vs GUI-action-path parity tests"
title: "TKT-012C: Cross-path parity tests (script vs GUI action path)"
labels: ["tkt-012", "frontend", "integration-tests"]
assignees: []
---

## Summary

Add integration tests proving equivalent intent yields equivalent canonical state across script execution and GUI action dispatch.

Reference: [plot3d_com_file/tickets.md](plot3d_com_file/tickets.md)

## Implementation Checklist

- [ ] Add integration harness to compare script final state to GUI action final state.
- [ ] Add round-trip cases for contour specs (automatic, increment, manual).
- [ ] Add round-trip cases for plot family and contour attribute selection.
- [ ] Add round-trip cases for VIEW/VPOINT and PLOT/UP orientation.
- [ ] Add round-trip cases for SUBSETS/WALLS and FSURFACE controls.
- [ ] Add round-trip cases for TEXT annotations and SHOW semantics where applicable.
- [ ] Assert parity at PlotState and RenderIntent boundaries (not pixel equivalence).

## Acceptance Criteria

- [ ] Script and GUI paths produce equivalent canonical outcomes for equivalent scenarios.
- [ ] Regressions fail with focused, debuggable diagnostics.

## Verification Evidence

- [ ] Include local integration test command/output.
- [ ] Link CI run that executes cross-path parity tests.
- [ ] Include one representative assertion diff format for failure readability.

## Out of Scope

- [ ] No parity matrix validator work (covered by TKT-012A).
- [ ] No required-check policy changes (covered by TKT-012D).
