# ADR-0004: Absolute Contour Value Model

- Status: Accepted
- Date: 2026-03-28

## Context

Legacy contour semantics use absolute physical values and multi-level definitions. A normalized single-value contour model (`0..1`) cannot represent these behaviors.

## Decision

Contour state is modeled with absolute values and multi-level capability.

Supported representation must include:

- Automatic mode
- Increment mode
- Manual absolute level entries

Normalized UI-only contour values are not authoritative state.

## Consequences

- Script and GUI contour behavior remain semantically aligned.
- Global field-range handling can be applied deterministically across multi-grid plots.
- Any adapter from normalized UI controls must convert into absolute model before commit.
