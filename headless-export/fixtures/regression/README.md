# Headless Regression Harness

This directory contains deterministic and semantic regression checks for headless export outputs.

## Checks Per Case

For each `.com` fixture, `check_regression.sh` executes two gates:

1. Deterministic hash gate
- Generate output PNG in `out/`.
- Compare generated SHA-256 against `reference/<case>.sha256`.

2. Semantic drift gate
- Compare generated PNG against `reference/<case>.png` using `overview-export-semantic-check`.
- Enforce tolerance thresholds for:
  - mean absolute RGB channel error
  - RMS RGB channel error
  - changed-pixel ratio
- Emit per-case semantic metrics into `out/semantic_metrics.txt` for triage and PR evidence.

Hash checks stay strict. Semantic checks provide meaningful-drift visibility with bounded tolerance.

## Default Semantic Thresholds

Defaults are configured in `check_regression.sh` and can be overridden with env vars:

- `SEM_MAX_MEAN_ERROR` (default: `0.75`)
- `SEM_MAX_RMS_ERROR` (default: `2.5`)
- `SEM_MAX_CHANGED_RATIO` (default: `0.005`)
- `SEM_CHANGED_THRESHOLD` (default: `8`)

Example override:

```bash
SEM_MAX_MEAN_ERROR=1.0 SEM_MAX_RMS_ERROR=3.0 headless-export/fixtures/regression/check_regression.sh
```

## Failure Triage

1. Hash mismatch
- Treat as deterministic drift.
- Inspect recent rendering logic, camera orientation changes, contour behavior, or reference assets.

2. Semantic threshold failure
- Treat as visual-drift tolerance breach.
- Inspect metrics emitted by the semantic checker and compare generated output with reference.

3. Intentional rendering change
- Update `reference/*.png` and matching `reference/*.sha256`.
- Record before/after semantic metrics from `out/semantic_metrics.txt` in PR notes so threshold decisions are explicit.
