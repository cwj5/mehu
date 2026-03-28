# Architecture Decision Records (ADR)

This directory records locked architecture decisions that protect parity behavior across script execution, GUI interaction, and export paths.

## Naming Convention

- File format: `NNNN-short-kebab-case-title.md`
- Example: `0001-shared-plotstate-authority.md`
- IDs are zero-padded and monotonically increasing.

## Required ADR Fields

Each ADR must include these top-level fields:

- `Status`: one of `Proposed`, `Accepted`, `Superseded`, `Deprecated`
- `Date`: ISO date (`YYYY-MM-DD`)
- `Decision`
- `Consequences`

## ADR Index

- [ADR-0001: Shared PlotState Authority](./0001-shared-plotstate-authority.md) (`Accepted`)
- [ADR-0002: PLOT Commit-Boundary Semantics](./0002-plot-commit-boundary.md) (`Accepted`)
- [ADR-0003: Unsupported Command Soft-Fail Policy](./0003-unsupported-command-soft-fail.md) (`Accepted`)
- [ADR-0004: Absolute Contour Value Model](./0004-absolute-contour-model.md) (`Accepted`)
- [ADR-0005: Deterministic Legacy-to-Modern Translation](./0005-deterministic-legacy-translation.md) (`Accepted`)
- [ADR-0006: Export Determinism and Divergence Policy](./0006-export-determinism-divergence-policy.md) (`Accepted`)
