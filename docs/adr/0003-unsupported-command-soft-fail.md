# ADR-0003: Unsupported Command Soft-Fail Policy

- Status: Accepted
- Date: 2026-03-28

## Context

Legacy command files include commands outside current support scope. Hard-failing entire script execution reduces usefulness and blocks incremental delivery.

## Decision

Unsupported commands must soft-fail:

- Emit a warning diagnostic with source location when available.
- Ignore only the unsupported command.
- Continue processing subsequent commands.

## Consequences

- Users receive actionable diagnostics without losing all script progress.
- Parity matrix must clearly track unsupported/script-only/gui-only states.
- Parser/executor behavior remains forward-compatible as support expands.
