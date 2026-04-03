# Legacy PLOT3D Terminology Migration - Agent Execution Plan

Date: 2026-04-02
Owner: Next implementation agent
Status: In progress (updated 2026-04-04)

## Progress Snapshot (2026-04-04 Final)

**All Phases 1-6 Complete**

Completed work:

1. ✅ **Phase 1**: Legacy terminology baseline and glossary in place (`terminology_glossary.md`) and referenced by user-facing docs.
2. ✅ **Phase 2**: Parser terminology/diagnostics alignment for legacy qualifier handling, including explicit warnings for deferred `CONTOURS/LINEAR`, `CONTOURS/CUBIC`, and `FSURFACE` qualifiers.
3. ✅ **Phase 3**: Shared state/API boundary text is divergence-explicit for bounded-MVP `FSURFACE` behavior.
4. ✅ **Phase 4**: Frontend terminology updated to legacy-facing labels for plot family (`SURFACE/CARPET/LINE`) and contour language.
5. ✅ **Phase 5**: Documentation/parity artifacts harmonized for bounded-MVP `FSURFACE` wording (`README.md`, `capability_catalog.md`, `parity_matrix.json`, `tickets.md`).
6. ✅ **Integration Testing**: Added comprehensive multi-command divergence warning coverage (4 fixtures, 3 new tests) verifying CONTOURS, FSURFACE, and VIEW deferred qualifiers surface in UI (all 29 integration tests passing).
7. ✅ **Internal Consistency Audit**: Verified naming alignment across function_surface/ContourAttribute enums, variable naming (camelCase/snake_case), and legacy terminology mapping. No breaking issues found.
8. ✅ **Phase 6 - Tests & CI Hardening**: Added 3 new regression tests validating:
   - Deferred CONTOURS qualifiers (LINEAR, CUBIC, RANGE) produce clear diagnostics
   - FSURFACE divergence messages use legacy terminology context (iso-level + FUNCTION)
   - Multiple command divergences show consistent legacy terminology
   - All 135 tests passing (132 base + 3 Phase 6)
   - All parity gates verified (parity-matrix OK, backend 1/1, integration 32/32)

Remaining optional work:

1. CLI parameter coverage for headless export (optional, Phase 7+)
2. Parser edge-case hardening (optional, Phase 7+)
3. Additional command coverage (WALLS, SUBSETS improvements) (optional, Phase 7+)

## Objective

Make terminology across parser, state/API boundaries, UI text, diagnostics, tests, and documentation reflect legacy PLOT3D language from [plot3d.md](../plot3d.md) and [plot3d.hlp](../plot3d.hlp).

This is primarily a terminology migration, with explicit handling for semantic divergences that cannot be fully behavior-aligned in the same pass.

## Ground Truth Sources

1. Legacy command semantics and wording:
- [plot3d.md](../plot3d.md)
- [plot3d.hlp](../plot3d.hlp)

2. Current implementation and contracts:
- [src-tauri/src/com_parser.rs](../src-tauri/src/com_parser.rs)
- [src-tauri/src/plot_state.rs](../src-tauri/src/plot_state.rs)
- [src-tauri/src/lib.rs](../src-tauri/src/lib.rs)
- [src/App.tsx](../src/App.tsx)
- [src/components/Viewer3D.tsx](../src/components/Viewer3D.tsx)
- [src/utils/solutionData.ts](../src/utils/solutionData.ts)

3. Governance and parity scope:
- [README.md](../README.md)
- [plot3d_com_file/capability_catalog.md](./capability_catalog.md)
- [plot3d_com_file/parity_matrix.json](./parity_matrix.json)
- [plot3d_com_file/legacy_translation_layer.md](./legacy_translation_layer.md)
- [docs/adr/0003-unsupported-command-soft-fail.md](../docs/adr/0003-unsupported-command-soft-fail.md)
- [docs/adr/0004-absolute-contour-model.md](../docs/adr/0004-absolute-contour-model.md)
- [docs/adr/0005-deterministic-legacy-translation.md](../docs/adr/0005-deterministic-legacy-translation.md)
- [docs/adr/0006-export-determinism-divergence-policy.md](../docs/adr/0006-export-determinism-divergence-policy.md)

## Current Findings You Must Preserve

## 1. Contour semantics are partially aligned

Aligned:
- Legacy CONTOURS modes (AUTOMATIC, INCREMENT, MANUAL) are implemented.
- Contour values are absolute physical values (not normalized 0..1), by design.

Partial/deferred:
- GRID and DOTS contour attributes are accepted but currently rendered via line fallback.
- LINEAR and CUBIC contour interpolation qualifiers are legacy terms that are not implemented end-to-end.

Key evidence:
- [plot3d.hlp](../plot3d.hlp#L1173)
- [plot3d.hlp](../plot3d.hlp#L1192)
- [src-tauri/src/com_parser.rs](../src-tauri/src/com_parser.rs#L518)
- [src-tauri/src/plot_state.rs](../src-tauri/src/plot_state.rs#L137)
- [src-tauri/src/lib.rs](../src-tauri/src/lib.rs#L2389)
- [src/components/Viewer3D.tsx](../src/components/Viewer3D.tsx#L747)
- [src/components/Viewer3D.tsx](../src/components/Viewer3D.tsx#L750)

## 2. FSURFACE is the highest-impact terminology/semantic drift

Legacy FSURFACE meaning:
- Function-surface plot property controls (scale factor, walls origin, contour-vs-grid line behavior).

Current code meaning:
- Iso-surface threshold spec with scalar field selection.

Key evidence:
- [plot3d.hlp](../plot3d.hlp#L1211)
- [plot3d.hlp](../plot3d.hlp#L1273)
- [src-tauri/src/plot_state.rs](../src-tauri/src/plot_state.rs#L373)
- [src-tauri/src/com_parser.rs](../src-tauri/src/com_parser.rs#L771)
- [src/App.tsx](../src/App.tsx#L2485)

## 3. PLOT family terminology is intentionally consolidated in code

- Legacy SURFACE, CARPET, and LINE are distinct user terms/synonyms.
- Current parser maps them to one internal family value (function_surface).

Key evidence:
- [plot3d.md](../plot3d.md#L338)
- [src-tauri/src/com_parser.rs](../src-tauri/src/com_parser.rs#L683)

## 4. Out-of-scope commands remain out-of-scope unless explicitly requested

- HELP, LIST, MAP, CLEAR, EXIT, QUIT, VECTORS, RAKES.

Key evidence:
- [README.md](../README.md#L45)
- [plot3d_com_file/parity_matrix.json](./parity_matrix.json#L84)

## Legacy Terminology Policy For This Migration

1. User-facing labels, help text, diagnostics, and docs should prefer legacy PLOT3D terms.
2. Internal enums/types may remain modern if needed for compatibility, but external contracts and visible text must present legacy naming.
3. If behavior does not yet match legacy semantics, wording must clearly indicate documented divergence.
4. No silent terminology drift: unsupported legacy qualifiers must be either handled or produce explicit diagnostic text.

## Term Mapping Baseline (Current -> Target)

1. function_surface (UI wording) -> Function Surface / Surface (Carpet/Line as legacy synonyms in context)
2. color_contours (UI copy) -> Color Contours
3. plot family copy should explicitly include legacy qualifier names:
- Contour
- Surface (Carpet)
- Line (2D degenerate Surface)
4. FSURFACE UI copy should use legacy definition wording, plus divergence note where behavior differs.
5. Contour qualifier copy should show legacy names verbatim:
- AUTOMATIC
- INCREMENT
- MANUAL
- RANGE
- LINEAR
- CUBIC
- ATTRIBUTES/NOATTRIBUTES
- LINE/SURFACE/GRID/COLOR/DOTS

## Execution Phases

## Phase 1: Canonical glossary and mapping freeze

Tasks:
1. Create a short glossary file in repo documenting approved legacy terms and approved internal aliases.
2. Include a do-not-use list for modernized user-facing synonyms.

Deliverable:
- New doc under plot3d_com_file, for example terminology_glossary.md.

Acceptance:
- Every migration change references this glossary.

## Phase 2: Parser and diagnostics terminology alignment

Primary files:
- [src-tauri/src/com_parser.rs](../src-tauri/src/com_parser.rs)

Tasks:
1. Ensure diagnostics use legacy command/qualifier wording.
2. Add explicit warnings for legacy qualifiers that are accepted in docs but not implemented in behavior (notably CONTOURS/LINEAR and CONTOURS/CUBIC).
3. Ensure FSURFACE diagnostics describe behavior in legacy-aware language and identify divergence when needed.

Acceptance:
- No ambiguous modern-only wording in parser diagnostics for in-scope legacy commands.

## Phase 3: Shared state and API boundary terminology

Primary files:
- [src-tauri/src/plot_state.rs](../src-tauri/src/plot_state.rs)
- [src-tauri/src/lib.rs](../src-tauri/src/lib.rs)

Tasks:
1. Keep internal invariants stable, but align exported comments/messages and command return payload descriptions with legacy terms.
2. Validate contour resolution command docs and outputs still describe absolute legacy semantics clearly.
3. For FSURFACE, either:
- implement adapter naming to legacy concept, or
- explicitly mark API field semantics as iso-surface divergence.

Acceptance:
- External command docs and diagnostics are legacy-consistent or divergence-explicit.

## Phase 4: Frontend terminology migration

Primary files:
- [src/App.tsx](../src/App.tsx)
- [src/components/Viewer3D.tsx](../src/components/Viewer3D.tsx)

Tasks:
1. Update visible labels and option text to legacy terms.
2. Update contour and function-surface notices to legacy language.
3. Replace ambiguous modern wording around function_surface with legacy qualifiers in helper text.
4. Add contextual text for FSURFACE divergence if behavior remains iso-surface-based.

Acceptance:
- Main UI path contains legacy term set consistently.

## Phase 5: Documentation and parity artifact migration

Primary files:
- [README.md](../README.md)
- [plot3d_com_file/parity_matrix.json](./parity_matrix.json)
- [plot3d_com_file/legacy_translation_layer.md](./legacy_translation_layer.md)

Tasks:
1. Align command and qualifier wording to legacy terminology policy.
2. Keep documented divergence sections explicit, especially FSURFACE and deferred contour attributes.
3. Refresh parity notes if wording changes affect scope interpretation.

Acceptance:
- Docs and code use same vocabulary; no contradictory phrasing.

## Phase 6: Tests and CI hardening

Primary files:
- Rust and TS test files that assert diagnostics/UI text.

Tasks:
1. Update test expectations for migrated wording.
2. Add regression coverage for:
- contour qualifier diagnostics (including LINEAR/CUBIC if still deferred),
- FSURFACE wording and divergence indication,
- plot family label wording.

Acceptance:
- Test suite and parity gates pass with updated terminology.

## Mandatory Checklist

1. Legacy command names are used in user-facing text for all in-scope capabilities.
2. Contour qualifiers are represented with legacy spellings in UI/docs/diagnostics.
3. PLOT family wording includes legacy SURFACE/CARPET/LINE context.
4. FSURFACE terminology either matches legacy semantics or is explicitly tagged as divergence in every user-facing location.
5. VIEW aliases TOP/SIDE/FRONT and XY/XZ/YZ style language are consistent in help text.
6. No silent unsupported qualifier behavior where a clear warning is feasible.
7. README, parity matrix notes, and translation layer docs use same terminology policy.
8. Tests reflect migrated language and pass.

## Suggested PR Strategy

PR 1 (low risk): terminology-only migration
- UI labels, docs, diagnostics wording, tests updates.
- No behavior changes.

PR 2 (medium risk): behavior-coupled alignments
- Qualifier warning additions (LINEAR/CUBIC etc).
- FSURFACE adapter/divergence handling improvements.
- Additional parity/test updates.

## Validation Commands

Run after each PR:
1. npm test
2. npm run test:coverage
3. npm run test:parity
4. npm run validate:parity-matrix

If headless path is relevant in changed code:
5. headless-export/fixtures/regression/check_regression.sh

## Risks and Guardrails

1. Risk: Breaking frontend-backend contract when renaming serialized fields.
- Guardrail: Prefer display-layer translation over wire-format renames unless coordinated.

2. Risk: Terminology-only PR accidentally changes behavior.
- Guardrail: Keep PR 1 behavior-neutral and enforce with focused tests.

3. Risk: FSURFACE confusion persists.
- Guardrail: Add one explicit, repeated divergence note until behavior is legacy-aligned.

## Handoff Notes For Next Agent

1. Start with Phase 1 glossary and get term map approved before touching code.
2. In each changed file, include a short comment or doc note only where needed to explain legacy terminology intent.
3. Update this plan doc with completion markers per phase as work proceeds.
4. If a term must stay modern internally, expose legacy wording at boundaries and document the mapping once in glossary.
