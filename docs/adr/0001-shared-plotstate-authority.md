# ADR-0001: Shared PlotState Authority

- Status: Accepted
- Date: 2026-03-28

## Context

Script execution and GUI interaction both mutate plotting behavior. Separate state machines cause drift and parity regressions.

## Decision

Use one backend-owned `PlotState` as the authoritative state model for supported plotting capabilities.

All supported mutations from either path must be represented as typed actions and applied through shared backend state transitions.

## Consequences

- Script and GUI parity is evaluated on the same state model.
- Feature work that bypasses `PlotState` is non-compliant.
- Testing can compare equivalent end states and commit outputs deterministically.
