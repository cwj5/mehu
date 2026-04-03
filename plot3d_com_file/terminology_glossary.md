# Legacy PLOT3D Terminology Glossary

Status: active terminology baseline for migration PRs
Last updated: 2026-04-02

## Purpose

This glossary defines the user-facing legacy PLOT3D terms that should be
preferred across docs, diagnostics, and UI text. Internal enum names and wire
formats may remain modern where needed, but external wording should follow this
baseline unless a documented divergence requires extra clarification.

## Approved User-Facing Terms

| Legacy term | Use it for | Internal alias or implementation note |
|---|---|---|
| `CONTOURS` | Level-selection command and contour attribute terminology | Backed by `ContourSpec` and `ContourAttribute` |
| `AUTOMATIC` | Automatically chosen contour levels | `ContourSpec::Automatic` |
| `INCREMENT` | Regular contour interval selection | `ContourSpec::Increment` |
| `MANUAL` | Explicit contour level entries | `ContourSpec::Manual` |
| `LINE` | Line contour rendering or 2D degenerate function-surface plot term | `ContourAttribute::Line`; `PLOT/LINE` maps to `PlotFamily::FunctionSurface` |
| `SURFACE` | Filled surface rendering or legacy function-surface qualifier | `ContourAttribute::Surface`; `PLOT/SURFACE` maps to `PlotFamily::FunctionSurface` |
| `CARPET` | Legacy synonym for function-surface plot family | Internal alias of `PlotFamily::FunctionSurface` |
| `COLOR CONTOURS` | Filled contour coloring mode | `ContourAttribute::ColorContours` |
| `GRID` | Grid-line contour attribute | `ContourAttribute::Grid`; currently first-pass fallback to `LINE` rendering |
| `DOTS` | Dot contour attribute | `ContourAttribute::Dots`; currently first-pass fallback to `LINE` rendering |
| `PLOT/CONTOUR` | Contour plot family | `PlotFamily::Contour` |
| `PLOT/SURFACE`, `PLOT/CARPET`, `PLOT/LINE` | Function-surface plot family terms | All map to `PlotFamily::FunctionSurface` |
| `FSURFACE` | Legacy function-surface property command name | Current implementation is a bounded MVP storing iso-level + FUNCTION |
| `FUNCTION` | Scalar-field selection by legacy number | Mapped through `map_legacy_function_number` |

## Divergence Notes

1. `CONTOURS/LINEAR` is accepted and diagnosed explicitly, but adds no extra behavior because current contour extraction already uses linear interpolation.
2. `CONTOURS/CUBIC` is accepted and diagnosed explicitly, but falls back to linear interpolation because cubic interpolation is not implemented.
3. `FSURFACE` currently represents an iso-level plus `FUNCTION` scalar-field selection, not the full legacy axis-property model (`SCALE_FACTOR`, `WALLS_ORIGIN`, `GRID`, `CONTOUR`).

## Do Not Use In User-Facing Text

Prefer the legacy wording above instead of these modernized phrases unless the text is explicitly documenting an internal alias or API field.

| Avoid in user-facing text | Prefer instead |
|---|---|
| `Function Surface` by itself | `SURFACE/CARPET/LINE` or explicit `PLOT/SURFACE (CARPET/LINE)` context |
| `plot mode` | `PLOT family` |
| `contour mode` | `CONTOURS Levels` or the specific legacy qualifier name |
| `iso-surface spec` without context | `FSURFACE` plus a divergence note if needed |
| `filled MVP` | `bounded MVP behavior` or an explicit legacy divergence statement |

## Usage Rule

When behavior differs from legacy PLOT3D, keep the legacy command name and add a concise divergence note rather than silently substituting modern terminology.