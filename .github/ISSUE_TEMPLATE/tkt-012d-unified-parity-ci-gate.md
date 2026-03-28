---
name: "TKT-012D - Unified Parity CI Gate"
about: "Implementation checklist for merge-blocking parity CI gate"
title: "TKT-012D: Unified parity CI gate"
labels: ["tkt-012", "ci", "quality-gate"]
assignees: []
---

## Summary

Wire parity governance and equivalence tests into a single required merge gate.

Reference: [plot3d_com_file/tickets.md](plot3d_com_file/tickets.md)

## Implementation Checklist

- [ ] Add or update workflow to run parity matrix validation, backend parity fixtures, and cross-path parity tests.
- [ ] Keep headless regression coverage in required checks policy.
- [ ] Ensure job naming clearly separates governance failures from behavior failures.
- [ ] Ensure workflow trigger coverage includes PRs to main/develop.
- [ ] Document required checks in repository docs.

## Acceptance Criteria

- [ ] PRs cannot merge when parity governance or parity-equivalence checks fail.
- [ ] Required check names are stable and contributor-friendly.

## Verification Evidence

- [ ] Link workflow file changes and resulting check names.
- [ ] Attach one intentionally failing run and one passing run.
- [ ] Confirm required-check configuration in repository settings.

## Out of Scope

- [ ] No new parity test logic except wiring/execution concerns.
- [ ] No ADR content authoring (covered by TKT-012E).
