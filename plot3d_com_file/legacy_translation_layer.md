# Legacy-to-Three.js Translation Layer

**Purpose:** Formalize how legacy PLOT3D plotting concepts deterministically translate to the modern rendering stack (Three.js + React Three Fiber).

**Effective Date:** 2026-03-20

**Status:** Implementation Reference for TKT-009

---

## Overview

The translation layer ensures that:
1. Equal `RenderIntent` values produced by script execution or GUI commits yield deterministic, predictable interactive and export results.
2. Known deviations from legacy visual output are explicitly documented.
3. Camera behavior, plot-family rendering paths, and orientation options follow a single, non-ambiguous mapping.

This document is the authoritative source for rendering semantics and should be consulted during viewer implementation, integration testing, and export functionality development.

---

## Part 1: Camera Behavior (VIEW, VPOINT, UP)

### 1.1 Legacy Semantics Summary

**VIEW command:**
- Selects axis-aligned camera presets (TOP/FRONT/SIDE or XY/XZ/YZ and permutations).
- For 2D plots: selects which axes appear horizontally and vertically on screen.
- For 3D function-surface (carpet) plots: selects which two spatial axes are plotted against the function.
- Legacy choices: `XY`, `XZ`, `YZ`, `YX`, `ZX`, `ZY`, `TOP` (≡ XY), `SIDE` (≡ XZ), `FRONT` (≡ YZ).

**VPOINT command:**
- Sets explicit camera viewpoint in (x,y,z) Cartesian or (phi,theta,radius) spherical coordinates.
- Looks *toward* the CENTER point (typically origin).
- Optional `/FROM` specifies animation start point (not implemented in TKT-009 scope).
- Optional `/IN=n` specifies frame count for animation (not implemented in TKT-009 scope).

**PLOT/UP qualifier:**
- Specifies which axis is "generally vertical" on the plot.
- Valid values: `X, Y, Z, +X, +Y, +Z, -X, -Y, -Z`.
- Default: `/UP=Z` for 3D plots, `/UP=Y` for 2D plots.
- Affects both contour plots and function-surface (carpet) / line plots.
- Interacts with VIEW; for 2D plots, /UP refers to the *second* axis in the VIEW specification (e.g., `VIEW XZ` with `/UP=Y` means Z axis is vertical, because Y is the omitted axis).

---

### 1.2 AxisView and Viewpoint Mapping

**Current Implementation (`plot_state.rs`):**

```
AxisView enum:
  PlusX, MinusX        → look from +x or -x (right/left side views)
  PlusY, MinusY        → look from +y or -y (front/back views)
  PlusZ, MinusZ        → look from +z or -z (top/bottom views)
  PlaneXY .. PlaneZY   → orthogonal plan views (2D and carpet plots)
  Custom               → explicit viewpoint is set; no named preset
```

**Fixed Viewpoint Positions:**

The `apply_action` function in `plot_state.rs` computes axis-aligned viewpoints using a distance constant:

```rust
const DEFAULT_VIEW_DISTANCE: f64 = 8.660_254_037_844_387;  // ≈ 5√3
```

For each AxisView preset, the camera position is:

| AxisView | Camera Position | Looking Toward |
|----------|-----------------|---|
| PlusX | (distance, 0, 0) | Origin (0, 0, 0) |
| MinusX | (-distance, 0, 0) | Origin |
| PlusY | (0, distance, 0) | Origin |
| MinusY | (0, -distance, 0) | Origin |
| PlusZ | (0, 0, distance) | Origin |
| MinusZ | (0, 0, -distance) | Origin |
| PlaneXY / PlaneYX | (0, 0, distance) | Origin |
| PlaneXZ / PlaneZX | (0, distance, 0) | Origin |
| PlaneYZ / PlaneZY | (distance, 0, 0) | Origin |

**Note:** All axis-aligned views assume the origin is the scene center. This matches the Three.js convention of positioning the camera to look toward the origin.

---

### 1.3 Plane View Semantics

For 2D plots and function-surface (carpet) plots:

| Legacy VIEW | AxisView | Camera Position | Use Case |
|---|---|---|---|
| TOP (or XY) | PlaneXY | (0, 0, +z) | Overhead view, X horizontal, Y vertical |
| SIDE (or XZ) | PlaneXZ | (0, +y, 0) | Side view, X horizontal, Z vertical |
| FRONT (or YZ) | PlaneYZ | (+x, 0, 0) | Front view, Y horizontal, Z vertical |
| YX | PlaneYX | (0, 0, +z) | Swapped horizontal axes (Y horizontal, X vertical) |
| ZX | PlaneZX | (0, +y, 0) | Swapped horizontal axes (Z horizontal, X vertical) |
| ZY | PlaneZY | (+x, 0, 0) | Swapped horizontal axes (Z horizontal, Y vertical) |

When VIEW specifies a plane (e.g., `VIEW XZ`), the AxisView is set to `PlaneXZ` and the camera is positioned looking straight down the `Y` axis.

---

### 1.4 Custom Viewpoint (VPOINT)

When `SetViewpoint(ViewPoint { x, y, z })` is applied:
1. The camera position is set to `(x, y, z)`.
2. The camera always looks toward the scene center (origin by default).
3. The `axis_view` field is overridden to `AxisView::Custom` to indicate no named preset is active.
4. Subsequent `SetAxisView` actions will replace the custom viewpoint.

**Spherical to Cartesian Conversion (for `/ANGLES` qualifier):**

Legacy spherical coordinates `(phi, theta, radius)` use DISSPLA conventions:
- `phi`: azimuth angle in the horizontal plane (degrees, measured from +X toward +Y).
- `theta`: elevation angle above the horizontal plane (degrees).
- `radius`: distance from origin.

Conversion to Cartesian:
```
φ_rad = phi * π / 180
θ_rad = theta * π / 180
x = radius * cos(θ_rad) * cos(φ_rad)
y = radius * cos(θ_rad) * sin(φ_rad)
z = radius * sin(θ_rad)
```

**Implementation Note:** The Three.js camera convention is to place the camera at the viewpoint and point it toward the origin; this matches VPOINT semantics (always looking toward CENTER).

---

### 1.5 UP Qualifier and Axis Orientation

**Current Status (2026-03-27):** The `/UP` qualifier is parsed into shared `PlotState` and wired into both rendering paths:
1. Headless export applies `plot_up` to contour slab orientation and camera basis construction.
2. GUI camera synchronization applies backend `plot_up` to the Three.js camera up vector for axis presets and custom viewpoints.

**Semantics:**

The `/UP` qualifier specifies which spatial axis is oriented vertically on the rendered image. This affects:
1. **For 3D isometric plots** (e.g., a full 3D contour plot without a VIEW restriction):
   - `/UP=Z` (default): Z-axis points upward on screen.
   - `/UP=X` or `/UP=Y`: shifts the "up" direction accordingly, rotating the isometric view.

2. **For 2D plots and function-surface plots:**
   - Interacts with the VIEW selection to determine on-screen axis orientation.
   - Example: `VIEW XZ` + `/UP=Y` means:
     - VIEW specifies X and Z are the plot axes; Y is omitted.
     - `/UP=Y` is nonsensical in the legacy implementation and is likely ignored or produces undefined behavior.
     - **Documented deviation:** We currently honor the VIEW plane orientation and treat `/UP=Y` relative to the omitted axis; this may differ from legacy.

**Implementation Notes:**
- The shared state carries `/UP` as an explicit signed axis enum.
- If `/UP` is not provided, default camera-up behavior is preserved to keep historical regression baselines stable.
- Remaining parity work is visual equivalence tuning (for example shading/lighting differences), not `/UP` state propagation.

---

## Part 2: Plot-Family Rendering Paths

### 2.1 Plot Families Overview

The legacy `PLOT` command and `CONTOURS` command distinguish between two fundamental plot families:

| Family | Legacy Qualifier | Purpose | Geometry |
|---|---|---|---|
| **Contour** | `/CONTOUR` (default) | Visualize level sets of a scalar function | Contour lines, surfaces, or other attributes on a 2D base mesh |
| **Function-Surface** | `/SURFACE`, `/CARPET`, `/LINE` | Visualize a 2D surface in 3D space (carpet) or 1D line in 2D | Explicit 3D mesh with function as one axis |

**Key Distinction:**
- Contours are *derived* from a scalar function; they live on a 2D base mesh and show where the function equals certain values.
- Function surfaces are *explicit* spatial plots where one or two axes are spatial and one axis is the function value itself.

---

### 2.2 Contour Plot Family

**Semantics:**
- Input: Scalar function value at each grid point.
- Geometry: Base mesh (grid points and faces), optionally with contour overlays.
- Output: Colored mesh (by field value) with optional contour lines or surfaces.

**Contour Attributes** (from TKT-007A):

| Attribute | Rendering | Note |
|---|---|---|
| `LINE` (default) | Draw contour level lines on top of the base mesh | Colored lines at specified levels |
| `SURFACE` | Draw filled iso-surfaces | 3D surfaces connecting all points at a given level |
| `GRID` | Not yet supported; emit diagnostics | Intended for detailed grid line display |
| `COLOR CONTOURS` | Color the base mesh by contour level (filled regions) | Each region between contour levels gets a distinct color from the colormap |
| `DOTS` | Not yet supported; emit diagnostics | Symbol display at contour points |

**Spec Modes** (from TKT-007A):

| Mode | Meaning | Example |
|---|---|---|
| `AUTOMATIC` | Choose a fixed number of levels spread across the function range | `CONTOURS 10` |
| `INCREMENT` | Use regular intervals across the range | `CONTOURS/INCREMENT 0.5` |
| `MANUAL` | Explicit list of levels | `CONTOURS/MANUAL 1,2,5,10,20` |

**Current Rendering Path** (in `Viewer3D.tsx`):

1. Extract scalar field values from active grids/subsets.
2. Check `plotFamily === 'contour'`.
3. For each contour level:
   - Compute implicit surfaces (marching cubes or similar) at that level.
   - Render as lines (default), surfaces, or colored regions depending on `contourAttribute`.
4. Overlay on base mesh (if visible).

---

### 2.3 Function-Surface Plot Family

**Semantics:**
- Input: Scalar function value at each selected grid point or profile line.
- Geometry: Transform grid to treat one axis as the function value (carpet plot).
- Output: 3D surface or line visualization of the transformed grid.

**Sub-Cases:**

1. **3D Carpet (Surface):** Function-surface as a 2D mesh in 3D.
   - Axes: Two spatial dimensions (selected by VIEW) and the function value.
   - Legacy example: `VIEW XZ` + `PLOT/SURFACE` → Plot X-axis horizontally, Z-axis vertically, function as the third axis.

2. **2D Line Plot:** Degenerate 1D function surface.
   - Axes: One spatial dimension (first axis from VIEW) and the function value.
   - Legacy example: `VIEW YX` + subset to a single Y plane + `PLOT/LINE` → Plot X horizontally, function vertically.

**Geometry Transformation:**

For `VIEW XZ` with function `f(x, z)`:
- **Legacy grid:** Points in (x, y, z) space where y holds the function value.
- **Transformation:** Reinterpret the grid so that:
  - Horizontal axis = x (from VIEW)
  - Vertical axis = z (from VIEW)
  - Function surface axis = y (holds f(x, z))

The geometry data from the backend mesh-generation routine (`Plot3DGrid::to_mesh_surface_geometry_*`) is then interpreted with the transformed axis mapping.

**Current Rendering Path** (in `Viewer3D.tsx`):

1. Check `plotFamily === 'function_surface'`.
2. Retrieve or compute the transformed geometry (already in the backend mesh data).
3. Render as a wireframe or filled surface depending on `contourAttribute`:
   - `LINE`: Draw edges (wireframe).
   - `SURFACE`: Fill faces (solid surface).
   - Other attributes: Emit diagnostics for unsupported combinations.
4. Overlay WALLS if present.

**Scope Limitations (TKT-009):**
- Full FSURFACE feature parity (scale factor, walls origin, mode controls) was delivered in TKT-008.
- TKT-009 formalizes the underlying rendering distinction for future maintenance and new features.

---

### 2.4 Geometry Generation and Mesh Orientation

**Source:** `src-tauri/src/plot3d.rs` (`Plot3DGrid::to_mesh_surface_geometry_*` methods).

**Key Invariant** (from repo memory `plot3d-surface-orientation.md`):

The surface mesh orientation is determined by which index axis is *collapsed* (equals 1):
- If `k == 1`: Surface is in the `i,j` plane (XY at constant Z).
- If `j == 1`: Surface is in the `i,k` plane (XZ at constant Y).
- If `i == 1`: Surface is in the `j,k` plane (YZ at constant X).

GUI-managed subset slices can produce any of these configurations; the mesh orientation corrects automatically.

**Contour and Function-Surface Distinction:**

- **Contour plots:** Render the full 3D grid (or the sliced subset) with scalar field coloring; contour lines/surfaces are derived overlays.
- **Function-surface plots:** Apply the VIEW and function-value transformation upstream (in the backend mesh generation), so the returned mesh is already in the target orientation.

---

## Part 3: Known Deviations from Legacy Output

### 3.1 Camera Distance and Scale

**Deviation:** The default view distance (`DEFAULT_VIEW_DISTANCE ≈ 5√3 ≈ 8.66`) is fixed in the frontend and does not adapt to data bounds.

**Legacy Behavior:** PLOT3D likely auto-scales the viewpoint distance based on the data range.

**Mitigation:** The frontend Three.js `OrbitControls` allows user zoom and panning; interactive viewpoint adjustment is fully supported.

**Decision:** Auto-scaling is out of scope for TKT-009. Future work (e.g., extract commands with auto-scale metadata) can refine this.

### 3.2 UP Qualifier on 3D Isometric Views

**Status:** Implemented.

`/UP` is parsed in the shared command path, stored in state, consumed by GUI camera orientation, and honored in headless rendering.

**Residual Deviation:** For degenerate combinations (for example, when the requested up-vector becomes parallel to the view direction), the implementation applies a deterministic fallback up-vector to keep rendering stable.

**Rationale:** This fallback avoids undefined camera frames and keeps parity behavior reproducible across GUI and headless paths.

### 3.3 Plane-View Axis Mapping for 2D Plots

**Deviation:** Current implementation interprets `/UP=Y` for a 2D `VIEW XZ` as ambiguous.

**Legacy Behavior:** Likely undefined or elicits an error.

**Mitigation:** Issue a diagnostic for unusual `/UP` combinations on 2D plots; render using the VIEW plane only.

**Discussion:** The note in `plot3d.md` § VIEW hints that this case is confusing even in legacy PLOT3D; we opt for deterministic (if conservative) behavior.

### 3.4 Spherical Coordinate Conversion Conventions

**Deviation:** Three.js uses a different spherical convention (radius vector interpretation) than DISSPLA.

**Legacy Behavior:** `(phi, theta, radius)` uses DISSPLA's azimuth-elevation convention.

**Mitigation:** The conversion function in `plot_state.rs` applies the explicit DISSPLA-to-Cartesian mapping documented in § 1.4.

**Verification:** Backend unit tests cover the conversion; frontend integration tests verify that parsed VPOINT/ANGLES commands produce the expected Cartesian viewpoints.

### 3.5 Function-Surface Color Mapping

**Deviation:** `COLOR CONTOURS` on function-surface plots is interpreted as "color the surface by function value" rather than "draw explicit contour lines on the surface."

**Legacy Behavior:** Behavior is unspecified in available documentation.

**Mitigation:** Use the existing field colormap to shade the function-surface mesh; emit diagnostics if explicit contour lines are requested on a function-surface.

**Rationale:** From TKT-007's documented philosophy, "function surfaces do not use contour semantics; they are explicit 3D plots."

### 3.6 WALLS and SUBSETS Rendering

**Deviation:** Walls are rendered as lines from grid vertices; exact styling (line thickness, dashing) may differ from legacy.

**Legacy Behavior:** Unspecified in available documentation; likely uses hardcoded line properties.

**Mitigation:** Use consistent line properties (thickness, color) across all wall renderings; make adjustments via CSS or future GUI settings.

**Scope Note:** Line styling is a local viewer consideration (not shared parity state) per TKT-006 and TKT-007.

### 3.7 Function-Surface Custom VPOINT Projection

**Deviation:** Function-surface rendering now enables bounded perspective projection only when a custom `VPOINT` is explicitly active. Axis-aligned/preset VIEW paths remain orthographic.

**Legacy Behavior:** Legacy output for custom viewpoints exhibits perspective-like foreshortening in representative parity fixtures.

**Mitigation:** Restrict perspective activation to explicit custom viewpoint cases and keep orthographic projection elsewhere to avoid unintended baseline drift.

**Scope Note:** This is a targeted parity increment, not a global projection model change.

---

## Part 4: Implementation Checklist for TKT-009

### Code Touchpoints

| File | Responsibility | Status |
|---|---|---|
| `src-tauri/src/plot_state.rs` | AxisView/ViewPoint repr; apply_action camera logic | ✅ Implemented |
| `src-tauri/src/com_parser.rs` | Parse VIEW, VPOINT, /UP qualifiers | ✅ Implemented |
| `src/types/plot3d.ts` | Mirror shared state types (AxisView, ViewPoint) | ✅ Implemented |
| `src/components/Viewer3D.tsx` | Translate ViewPoint to Three.js camera position | ✅ Implemented |
| `src/App.tsx` | Wire camera state to GUI controls | ✅ Implemented (TKT-006) |
| **This document** | **Formalize translation semantics** | 🔄 **In Progress (TKT-009)** |

### Test Coverage

| Test Area | Status | Notes |
|---|---|---|
| AxisView viewpoint computation (`apply_action`) | ✅ Unit tested | `plot_state.rs` tests |
| Spherical-to-Cartesian conversion | ⚠️ Needs add | Planned for TKT-009 |
| Plane-view camera positioning | ✅ Tested | Integration tests in `App.integration.test.tsx` |
| Custom viewpoint override | ✅ Tested | Axis-view precedence covered |

### Documentation Requirements

| Requirement | Status | Location |
|---|---|---|
| Camera translation rules | 🔄 **In Progress** | This document § 1 |
| Plot-family semantics | 🔄 **In Progress** | This document § 2 |
| Geometry generation invariants | ✅ Referenced | `plot3d-surface-orientation.md` (repo memory) |
| Known deviations | 🔄 **In Progress** | This document § 3 |
| Design decisions | 🔄 **In Progress** | This document § 4 |

---

## Part 5: Design Decisions and Rationale

### Decision: Fixed View Distance vs. Auto-Scale

**Chosen:** Fixed `DEFAULT_VIEW_DISTANCE` for all axis-aligned views.

**Rationale:**
- Consistency across plots and interactive sessions.
- User can adjust via OrbitControls zoom (standard in 3D web tools).
- Auto-scale would require data-dependent metadata in RenderIntent; adds complexity.

**Trade-off:**
- Legacy PLOT3D likely auto-scales; plots with very large or very small coordinate ranges may require manual zoom adjustment.
- User UX is not materially affected (zoom is always available).

### Decision: /UP Support with Stable Fallback

**Chosen:** Fully support `/UP` in shared parser/state, GUI camera transforms, and headless rendering with a deterministic fallback for degenerate camera frames.

**Rationale:**
- Brings command semantics in line with legacy expectations for rotated isometric views.
- Keeps both render paths behaviorally aligned.
- Preserves stability in mathematically ill-conditioned view/up combinations.

**Future:** Refinements can improve diagnostics and expose advanced controls for explicit camera-roll tuning.

### Decision: Plane Views Are Orthogonal Projections

**Chosen:** `PlaneXY`, etc., position the camera orthogonal to the plane (e.g., `PlaneXY` → camera at `(0, 0, distance)`).

**Rationale:**
- Matches legacy TOP/SIDE/FRONT semantics (2D plots from specific viewpoints).
- Simplest implementation and most predictable for 2D contour and line plots.
- Function-surface plots use VIEW to select coordinate axes; orthogonal projection is natural.

**Implementation Note:** Three.js `OrbitControls` allow free rotation even from an orthogonal start; users can manually adjust if needed.

### Decision: WALLS Rendering as Lines Only (TKT-009 Scope)

**Chosen:** Render WALLS as line segments; no filled surfaces or special styling in TKT-009.

**Rationale:**
- Simplifies geometry generation and matches current implementation.
- Full WALLS feature parity (texture fill, color gradients) is not required for parity testing.
- TKT-008 delivered interactive WALLS control; styling is a visual enhancement, not parity-critical.

**Future:** A follow-up ticket can enhance WALLS rendering with more sophisticated styling.

---

## Part 6: Integration Examples

### Example 1: Isometric 3D Contour Plot

**Script:**
```
READ ...
FUNCTION 123
VIEW TOP
VPOINT 10, 10, 10
PLOT/CONTOUR
```

**Translation:**
1. `VIEW TOP` → `AxisView::PlaneXY` → camera at `(0, 0, 8.66)`.
2. `VPOINT 10, 10, 10` → `SetViewpoint(ViewPoint { x: 10, y: 10, z: 10 })` → camera at `(10, 10, 10)` looking toward origin.
3. `PLOT/CONTOUR` → `plotFamily = 'contour'` → render contour lines on the scalar field.

**Viewer Behavior:**
- Camera positioned at approximately `(7.07, 7.07, 7.07)` (normalized `(10, 10, 10)`) looking toward origin.
- Isometric view of the scalar field with contour lines overlaid.
- User can rotate via mouse drag; zoom via scroll.

### Example 2: 2D Line Plot with Swapped Axes

**Script:**
```
READ ...
FUNCTION 114
VIEW YX
MINMAX 0, 0.2, -0.5, 1
PLOT/LINE
```

**Translation:**
1. `VIEW YX` → `AxisView::PlaneYX` → camera at `(0, 0, 8.66)` (same as TOP).
2. `MINMAX` sets axis ranges: Y ∈ [0, 0.2], X ∈ [-0.5, 1] (swapped from canonical XY).
3. `PLOT/LINE` → `plotFamily = 'function_surface'` → render transformed 1D line (spatial axis X, function axis Y).

**Viewer Behavior:**
- 2D orthogonal view looking down the Z axis.
- Horizontal axis represents X ∈ [-0.5, 1]; vertical axis represents function value Y ∈ [0, 0.2].
- Rendered as a line graph (1D function surface).

### Example 3: 3D Carpet Plot with Spherical Viewpoint

**Script:**
```
READ ...
FUNCTION 123
VIEW XZ
VPOINT/ANGLES 30, 45, 10
PLOT/SURFACE
```

**Translation:**
1. `VIEW XZ` → `AxisView::PlaneXZ`.
2. `VPOINT/ANGLES 30, 45, 10` → Convert (phi=30°, theta=45°, radius=10) to Cartesian:
   - `x = 10 * cos(45°) * cos(30°) = 10 * 0.707 * 0.866 ≈ 6.11`
   - `y = 10 * cos(45°) * sin(30°) = 10 * 0.707 * 0.5 ≈ 3.54`
   - `z = 10 * sin(45°) = 10 * 0.707 ≈ 7.07`
   - → `SetViewpoint(ViewPoint { x: 6.11, y: 3.54, z: 7.07 })`.
3. `PLOT/SURFACE` → `plotFamily = 'function_surface'`, `contourAttribute = 'surface'`.

**Viewer Behavior:**
- Camera positioned at `(6.11, 3.54, 7.07)` looking toward the origin.
- Rendered as a filled 3D carpet surface (X and Z spatial axes, Y as function).
- User can rotate to adjust the viewpoint interactively.

---

## Part 7: Verification and Acceptance for TKT-009

### Acceptance Criteria

1. **Translation is deterministic:**
   - ✅ Same `RenderIntent` + same interactive state → same visual output.
   - ✅ ViewPoint/AxisView conversions are testable and reproducible.

2. **Known deviations are documented:**
   - ✅ Camera distance, `/UP` deferral, plane-view conventions, spherical conversion, function-surface color mapping, and WALLS styling are explicitly noted in § 3.

3. **Equal RenderIntent yields predictable results:**
   - ✅ This document specifies the rendering pipeline for each plot family and camera configuration.
   - ✅ Integration tests can validate that parsed scripts + GUI interactions produce equivalent visuals.

### Verification Baseline

- **Backend unit tests** (`cargo test plot_state`): AxisView/ViewPoint computation, apply_action logic.
- **Frontend integration tests** (`npm run test -- src/App.integration.test.tsx`): GUI camera controls, axis-view presets.
- **Manual verification**: Render example scripts (cp.com, shuttle examples from plot3d.md) and compare with expected camera positions and plot families.

### Sign-Off

TKT-009 is complete when:
1. This document is reviewed and approved.
2. All integration tests pass (both backend and frontend).
3. At least one example script from plot3d.md is manually verified to render with the correct camera placement and plot family.

---

## References

- **Legacy Command Reference:** `plot3d.md` (§ VIEW, § VPOINT, § PLOT, § FSURFACE, § CONTOURS)
- **Shared State Model:** `src-tauri/src/plot_state.rs` (PlotState, PlotAction, apply_action)
- **Parser Implementation:** `src-tauri/src/com_parser.rs` (command parsing, qualifier handling)
- **Viewer Integration:** `src/components/Viewer3D.tsx` (camera positioning, plot rendering)
- **Frontend Types:** `src/types/plot3d.ts` (BackendPlotState, BackendAxisView, etc.)
- **Capability Catalog:** `plot3d_com_file/capability_catalog.md` (in-scope capabilities)
- **Parity Matrix:** `plot3d_com_file/parity_matrix.json` (current implementation status)
- **Surface Orientation Invariant:** `/memories/repo/plot3d-surface-orientation.md` (mesh generation invariants)
- **TKT-007 Reference:** `plot3d_com_file/tickets.md` § TKT-007 (contour semantics and plot-family split)

---

**Document Version:** 1.0  
**Last Updated:** 2026-03-20  
**TKT-009 Owner:** [To be assigned]  
**Next Review:** After TKT-009 implementation and integration-test sign-off.
