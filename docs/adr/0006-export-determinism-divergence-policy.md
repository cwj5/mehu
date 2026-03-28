# ADR-0006: Export Determinism and Divergence Policy

- Status: Accepted
- Date: 2026-03-28

## Context

In-app export and headless export use different execution environments and may not be pixel-identical. Without policy, drift becomes difficult to reason about.

## Decision

Primary parity target is deterministic equivalence of `PlotState` and `RenderIntent`.

Export policy:

- Favor shared render-intent semantics across GUI and headless paths.
- Document known visual divergence explicitly.
- Guard against unintended regressions with deterministic fixture and regression checks.

## Consequences

- Teams can evolve rendering internals while preserving stable intent behavior.
- CI gates detect regressions at semantic and export-regression levels.
- Contributors must update divergence docs when behavior intentionally differs.
