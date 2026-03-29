# Testing & Coverage Guide

This document outlines the testing infrastructure and coverage requirements for the overview project.

## Quick Start

### Initial Setup
After cloning the repository, set up the pre-commit hooks:

```bash
bash setup-hooks.sh
```

This configures git to automatically run tests before each commit.

### Running Tests

```bash
# Run the parity gate locally
npm run test:parity

# Run parity matrix governance only
npm run validate:parity-matrix

# Run backend parity fixtures only
npm run test:parity-backend

# Run frontend cross-path parity only
npm run test:parity-cross-path

# Run all TypeScript tests
npm test

# Run TypeScript tests with coverage
npm run test:coverage

# Watch mode for development
npm run test:watch

# Run Rust backend tests
cd src-tauri && cargo test --lib

# Run headless regression check required by branch protection
cd .. && headless-export/fixtures/regression/check_regression.sh
```

## Required Parity Checks

Parity status snapshot (from `plot3d_com_file/parity_matrix.json`, lastUpdated `2026-03-28`): all 12 in-scope capabilities are currently marked `supported`. `FUNCTION` includes bounded soft-fail diagnostics for known unimplemented legacy equations.

The repository's parity policy is split into stable CI checks so contributors can diagnose failures quickly:

- `Parity Governance`: validates `plot3d_com_file/parity_matrix.json` freshness and governance rules.
- `Backend Parity Fixtures`: runs the Rust fixture-based parity equivalence test in `src-tauri/src/script_executor.rs`.
- `Cross-Path Parity`: runs the frontend integration parity coverage in `src/App.integration.test.tsx`.
- `Headless Regression`: runs deterministic hash checks plus tolerance-based semantic image checks in `.github/workflows/headless-regression.yml`.

The first three checks are defined in [.github/workflows/parity-matrix.yml](.github/workflows/parity-matrix.yml). `Headless Regression` remains a separate required workflow because export-path drift is intentionally tracked outside the GUI/script parity suite.

## Parity Validation Sequence (Local)

Use this exact sequence from repo root to mirror required checks:

```bash
npm run validate:parity-matrix
npm run test:parity-backend
npm run test:parity-cross-path
headless-export/fixtures/regression/check_regression.sh
```

Expected success indicators:

- `validate:parity-matrix`: `[parity-matrix] OK: ... capability rows validated`
- `test:parity-backend`: `backend_parity_fixtures_match_expected_outputs ... ok`
- `test:parity-cross-path`: `Test Files  1 passed` and all parity tests green
- `check_regression.sh`: hash comparison and semantic threshold checks both pass with no errors

## When PRs Must Update Parity Matrix

Update [plot3d_com_file/parity_matrix.json](plot3d_com_file/parity_matrix.json) in the same PR when you change:

- capability support status
- ticket linkage, rationale, or notes for any capability row
- parser/executor behavior or GUI behavior that affects parity capability semantics
- parity scope assumptions in docs that would make matrix metadata stale

The parity governance check enforces freshness and will fail if capability-affecting code changed after `lastUpdated`.

## Parity Troubleshooting

- `Parity Governance` failure:
  - Run `npm run validate:parity-matrix` locally.
  - Fix schema/governance errors and refresh `lastUpdated` after reviewing parity impact.
- `Backend Parity Fixtures` failure:
  - Run `npm run test:parity-backend` and inspect expected vs actual fixture outputs in backend tests.
- `Cross-Path Parity` failure:
  - Run `npm run test:parity-cross-path` and inspect the failing case in `src/App.integration.test.tsx`.
- `Headless Regression` failure:
  - Re-run `headless-export/fixtures/regression/check_regression.sh` and compare generated outputs under `headless-export/fixtures/regression/out`.
  - If hashes fail, treat as deterministic drift (reference update or behavior regression) and inspect script/rendering changes first.
  - If hashes pass but semantic checks fail, treat as tolerance drift and inspect camera/orientation, contour setup, precision changes, and image dimensions.
  - Use semantic thresholds from `check_regression.sh` (`SEM_MAX_MEAN_ERROR`, `SEM_MAX_RMS_ERROR`, `SEM_MAX_CHANGED_RATIO`, `SEM_CHANGED_THRESHOLD`) to determine whether drift is intentional.
  - For intentional drift, update references and include before/after metrics in the PR notes.

## Test Structure

### TypeScript/Frontend Tests

Located in `src/utils/*.test.ts`

**Test Frameworks:**
- Vitest 3.0.0 - Test runner
- @vitest/coverage-v8 - Coverage reporting

**Coverage:**
- ✅ 100 tests
- ✅ 97.62% coverage in src/utils
- ✅ Modules: gridUtils (100%), shaderMaterials (100%), solutionData (98.73%), colorMapping (99.3%), logger (90.82%)

**Key Test Suites:**
- `gridUtils.test.ts` - Grid visibility and grouping
- `colorMapping.test.ts` - Color scheme validation and mapping
- `solutionData.test.ts` - CFD field computations
- `shaderMaterials.test.ts` - Three.js material creation
- `logger.test.ts` - Frontend logging system

### Rust Backend Tests

Located in `src-tauri/src/**_tests.rs`

**Test Framework:**
- Cargo built-in test framework
- Tarpaulin for coverage reporting

**Coverage:**
- ✅ 86 tests
- ✅ 45.28% overall coverage
- ✅ Modules: logger (95.56%), solution (95.65%), plot3d (54.04%), lib (0% - FFI bindings only)

**Key Test Modules:**
- `logger_tests.rs` - Logging functionality
- `plot3d.rs::tests` - File parsing and mesh generation
- `solution.rs::tests` - CFD field computations and color mapping

## Pre-commit Hooks

The `.githooks/pre-commit` script runs:

1. **TypeScript Tests**: `npm test`
2. **Rust Tests**: `cargo test --lib` (in src-tauri)

If either test suite fails, the commit is prevented.

### Bypassing Hooks (Not Recommended)

```bash
git commit --no-verify
```

## GitHub Actions Workflows

### Build and Release Pipeline
- **File**: `.github/workflows/build.yml`
- **Trigger**: Push to main/develop, tags with 'v*'
- **Jobs**: Build for Linux, macOS, Windows
- **Artifacts**: Binary releases for all platforms

### Test and Coverage Pipeline
- **File**: `.github/workflows/test-coverage.yml`
- **Trigger**: All push and PR events
- **Jobs**: 
  - TypeScript tests with coverage upload to Codecov
  - Rust tests with coverage upload to Codecov
  - Coverage report generation and PR comments

### Parity Gate Pipeline
- **File**: `.github/workflows/parity-matrix.yml`
- **Trigger**: Push to `main`/`develop`/`plot3d-com-file` and pull requests to `main`/`develop`
- **Stable Checks**:
  - `Parity Governance`
  - `Backend Parity Fixtures`
  - `Cross-Path Parity`

### Headless Regression Pipeline
- **File**: `.github/workflows/headless-regression.yml`
- **Trigger**: Push to `main`/`develop`/`plot3d-com-file` and pull requests to `main`/`develop`
- **Stable Check**: `Headless Regression`

## Coverage Goals

### Target Coverage Levels

| Component | Target | Current | Status |
|-----------|--------|---------|--------|
| TypeScript (utils) | 95%+ | 97.62% | ✅ |
| Rust (backend) | 50%+ | 45.28% | 🟡 |
| Overall | 80%+ | 71% | 🟡 |

### Recently Improved

- **solution.rs**: 50.43% → 95.65% (+45.22%)
- **logger.rs**: 66.67% → 95.56% (+28.89%)
- **colorMapping.ts**: 76.22% → 99.3% (+23.08%)

## Adding New Tests

### TypeScript Tests

```typescript
import { describe, it, expect } from 'vitest';
import { myFunction } from './myModule';

describe('myModule', () => {
  it('should do something', () => {
    const result = myFunction(input);
    expect(result).toBe(expected);
  });
});
```

Test files should:
- Be placed next to the module they test
- Use `.test.ts` extension
- Cover happy path, edge cases, and error scenarios
- Maintain > 90% coverage per module

### Rust Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        let result = my_function();
        assert_eq!(result, expected);
    }
}
```

Rust tests should:
- Be in `mod tests` blocks with `#[cfg(test)]`
- Be placed in the same file as the code they test
- Cover unit-level functionality
- Use `#[test]` attribute for each test function

## Coverage Measurement

### TypeScript Coverage

Generated with Vitest:

```bash
npm run test:coverage
# Output: coverage/
```

### Rust Coverage

Generated with Cargo Tarpaulin:

```bash
cd src-tauri
cargo tarpaulin --lib --timeout 300 --out Xml
# Output: cobertura.xml for CI/CD integration
```

## Troubleshooting

### Tests Fail in Pre-commit Hook

If tests fail before a commit:

1. Review the error message
2. Run tests locally: `npm test` or `cargo test --lib`
3. Fix the failing tests
4. Stage and commit again

### Coverage Drop

If coverage decreases:

1. Check which files/lines lost coverage
2. Add tests for new functionality
3. Review refactored code for adequate test coverage

### GitHub Actions Failures

Check the Actions tab in the GitHub repository to view:
- Test output logs
- Coverage reports
- Build artifacts

## Resources

- [Vitest Documentation](https://vitest.dev/)
- [Cargo Testing](https://doc.rust-lang.org/cargo/commands/cargo-test.html)
- [Tarpaulin Coverage](https://github.com/tafia/cargo-tarpaulin)
- [GitHub Actions Documentation](https://docs.github.com/actions)
