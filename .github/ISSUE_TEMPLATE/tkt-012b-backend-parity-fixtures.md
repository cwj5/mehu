---
name: "TKT-012B - Backend Parity Fixtures"
about: "Implementation checklist for backend parity-equivalence fixture suite"
title: "TKT-012B: Backend parity-equivalence fixture suite"
labels: ["tkt-012", "backend", "tests"]
assignees: []
---

## Summary

Add deterministic fixture-driven backend parity tests for parser, state transitions, and render-intent output.

Reference: [plot3d_com_file/tickets.md](plot3d_com_file/tickets.md)

## Implementation Checklist

- [ ] Add fixture set for representative capability scenarios.
- [ ] Add Rust tests asserting deterministic final PlotState for each fixture.
- [ ] Add Rust tests asserting deterministic RenderIntent shape and values.
- [ ] Cover FUNCTION, VIEW/VPOINT, MINMAX, CONTOURS, PLOT, WALLS, SUBSETS, FSURFACE, TEXT, SHOW.
- [ ] Add edge-case fixtures for PLOT/UP orientation and contour mode variants.
- [ ] Add edge-case coverage for thin-slab function-surface behavior.
- [ ] Ensure tests are deterministic across Linux/macOS/Windows CI.

## Acceptance Criteria

- [ ] Fixture suite fails on parser/state/render-intent behavior drift.
- [ ] Tests are stable and deterministic in CI.
- [ ] Capability changes require fixture updates or explicit test rationale.

## Verification Evidence

- [ ] Include local test command and pass/fail output.
- [ ] Link CI run with backend parity fixture job.
- [ ] List fixtures added and which capability each fixture covers.

## Out of Scope

- [ ] No GUI action-path equivalence checks here (covered by TKT-012C).
- [ ] No parity workflow wiring changes here (covered by TKT-012D).
