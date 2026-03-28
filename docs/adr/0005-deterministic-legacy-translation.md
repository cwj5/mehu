# ADR-0005: Deterministic Legacy-to-Modern Translation

- Status: Accepted
- Date: 2026-03-28

## Context

Legacy PLOT3D conventions (VIEW/VPOINT/UP/FUNCTION and qualifiers) do not map one-to-one with modern Three.js rendering primitives. Implicit or ad-hoc translation causes inconsistent behavior.

## Decision

Use an explicit, deterministic translation layer that maps legacy semantics into modern state and rendering conventions.

Translation rules must be stable, documented, and shared by script and GUI paths.

## Consequences

- Equivalent inputs produce equivalent backend state and intent outputs.
- Known deviations from legacy visuals are documented, not accidental.
- Future changes to translation behavior require explicit ADR or amendment.
