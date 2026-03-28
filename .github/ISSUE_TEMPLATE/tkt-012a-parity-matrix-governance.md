---
name: "TKT-012A - Parity Matrix Governance"
about: "Implementation checklist for parity matrix governance and freshness checks"
title: "TKT-012A: Parity matrix governance and freshness checks"
labels: ["tkt-012", "governance", "ci"]
assignees: []
---

## Summary

Implement governance checks that prevent parity-matrix drift beyond JSON-schema validation.

Reference: [plot3d_com_file/tickets.md](plot3d_com_file/tickets.md)

## Implementation Checklist

- [ ] Extend [scripts/validate-parity-matrix.mjs](scripts/validate-parity-matrix.mjs) with semantic governance checks.
- [ ] Enforce valid ticket reference format for each capability row.
- [ ] Require non-empty `notes` for all capability rows.
- [ ] Require explicit rationale text when status is `script-only` or `gui-only`.
- [ ] Add `lastUpdated` freshness policy tied to capability-affecting file changes.
- [ ] Add actionable error output with clear fix guidance.
- [ ] Ensure [.github/workflows/parity-matrix.yml](.github/workflows/parity-matrix.yml) fails on governance violations.

## Acceptance Criteria

- [ ] CI fails on stale or semantically incomplete parity rows.
- [ ] Capability status drift cannot merge without metadata updates.
- [ ] Validator failures point to exact row and exact missing field/reason.

## Verification Evidence

- [ ] Include command output for local validator run.
- [ ] Include one failing example (before) and one passing example (after).
- [ ] Link PR checks showing parity-matrix workflow status.

## Out of Scope

- [ ] No capability status changes unless explicitly intended by the issue.
- [ ] No ADR/doc rewrites in this issue.
