---
name: "TKT-012F - Parity Docs Hardening"
about: "Implementation checklist for README/TESTING parity support and limitations"
title: "TKT-012F: Documentation hardening for parity support and limitations"
labels: ["tkt-012", "docs", "onboarding"]
assignees: []
---

## Summary

Harden top-level docs so contributors can understand supported scope, known limitations, and parity workflows quickly.

Reference: [plot3d_com_file/tickets.md](plot3d_com_file/tickets.md)

## Implementation Checklist

- [ ] Update README supported-scope section from parity matrix.
- [ ] Document known limitations and link legacy translation-layer divergence notes.
- [ ] Update TESTING with local parity commands and expected outputs.
- [ ] Document which PR changes require parity-matrix updates.
- [ ] Document required CI parity checks and troubleshooting notes.

## Acceptance Criteria

- [ ] Supported scope and known limitations are clear from top-level docs.
- [ ] New contributors can run parity checks locally using docs only.
- [ ] Docs remain aligned with CI-required parity checks.

## Verification Evidence

- [ ] Link README sections added/updated.
- [ ] Link TESTING sections added/updated.
- [ ] Include copy of local parity command sequence used for validation.

## Out of Scope

- [ ] No new test implementation unless needed for docs accuracy.
- [ ] No CI policy wiring changes except doc references.
