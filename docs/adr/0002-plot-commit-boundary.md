# ADR-0002: PLOT Commit-Boundary Semantics

- Status: Accepted
- Date: 2026-03-28

## Context

Legacy `.com` files accumulate plotting configuration before committing a render step. Without an explicit commit boundary, side effects become ambiguous.

## Decision

Treat `PLOT` as the commit boundary:

- Actions before `PLOT` mutate pending `PlotState` only.
- `PLOT` emits a `RenderIntent` snapshot derived from the current `PlotState`.
- Multi-`PLOT` scripts produce one ordered `RenderIntent` per commit.

## Consequences

- Render/export systems consume stable intent snapshots.
- GUI parity should commit at meaningful apply boundaries, not each transient edit frame.
- Tests can assert deterministic intent sequences.
