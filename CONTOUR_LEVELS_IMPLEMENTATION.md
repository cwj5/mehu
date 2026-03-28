# Contour Levels Implementation Plan

**Date:** March 7, 2026  
**Status:** Planning Complete - Ready for Implementation

## Overview

This document describes the implementation plan for adding contour level visualization to the Overview application. The feature consists of two main components:
1. **Volume contour iso-surfaces** - 3D surfaces at constant scalar field values
2. **Contour lines on slice and arbitrary planes** - 2D contour lines overlaid on existing slice geometries

## User Requirements & Decisions

### Scope
- **Level Strategy:** Single manual contour level (MVP)
- **Range Normalization:** Global field range (matches current color behavior)
- **IBLANK Filtering:** Respect existing IBLANK/fringe/filter mode settings
- **UI Controls:** Toggle on/off + manual level input only

### Visual Appearance
- **Iso-Surface Styling:**
  - Solid single color for MVP
  - Per-grid extraction (no cross-grid merging)
  - Future enhancement: color by separate scalar field
  
- **Contour Line Styling:**
  - Colored contour lines
  - Underlying slice/plane mesh switches to muted grey when contours enabled
  - When contours disabled, slice mesh returns to field coloring

- **Coexistence:** Volume iso-surfaces and slice/plane contours can render simultaneously

### Behavior
- **Default Level:** 0.5 normalized (mid-range between min/max)
- **Out-of-Range Input:** Auto-clamp to [0,1] range + display warning to user
- **Multi-Grid:** Each grid gets its own iso-surface (extracted separately)

## Technical Architecture

### Current Codebase Context

#### Data Model & State Management
- **Types:** [src/types/plot3d.ts](src/types/plot3d.ts) (`Plot3DGrid`, `Plot3DSolution`, `GridMetadata`, `SolutionMetadata`)
- **Types:** [src/types/grids.ts](src/types/grids.ts) (`GridItem`, `GridSlice`, `ArbitrarySlice`)
- **App State:** [src/App.tsx](src/App.tsx) manages `gridSlices`, `arbitrarySlices`, `currentScalarField`, `currentColorScheme`, IBLANK settings

#### Rendering Pipeline
- **Frontend Renderer:** [src/components/Viewer3D.tsx](src/components/Viewer3D.tsx)
  - `SolidMeshRenderer` for surface meshes
  - `MeshRenderer` for various geometry types
  - Triggers recompute on state changes (slices, fields, colors, IBLANK)
  
- **Backend Commands:** [src-tauri/src/lib.rs](src-tauri/src/lib.rs)
  - `load_plot3d_file_cached`, `load_plot3d_solution_cached`
  - `slice_grid_by_id`, `slice_arbitrary_plane_by_id`
  - `compute_solution_colors_sliced`, `compute_solution_colors_arbitrary_plane`
  - `get_solution_field_range`

- **Geometry Kernels:** [src-tauri/src/plot3d.rs](src-tauri/src/plot3d.rs)
  - `Plot3DGrid::slice_grid` - I/J/K slicing
  - `slice_arbitrary_plane_with_solution` - arbitrary plane extraction
  - `to_mesh_surface_geometry_decimated*` - surface mesh generation
  - `MeshGeometry`, `VertexCellData` structures

- **Scalar/Color Kernels:** [src-tauri/src/solution.rs](src-tauri/src/solution.rs)
  - `ScalarField`, `ColorScheme` enums
  - `compute_scalar_field_surface` - per-vertex scalar computation
  - `compute_colors_with_range` - colormap application
  - `map_value_to_color` - individual value mapping

#### Existing Reusable Components
- **Slice Infrastructure:** Already produces triangulated surfaces with per-vertex scalar values
- **Arbitrary Plane Infrastructure:** Robust welded triangulation + interpolation weights
- **Color Mapping:** [src/utils/colorMapping.ts](src/utils/colorMapping.ts) + [src-tauri/src/solution.rs](src-tauri/src/solution.rs)
- **Shader Materials:** [src/components/Viewer3D.tsx](src/components/Viewer3D.tsx) supports colored lines + solid rendering

#### Testing Infrastructure
- **TypeScript Tests:**
  - [src/utils/solutionData.test.ts](src/utils/solutionData.test.ts)
  - [src/utils/colorMapping.test.ts](src/utils/colorMapping.test.ts)
  - [src/utils/shaderMaterials.test.ts](src/utils/shaderMaterials.test.ts)

- **Rust Tests:**
  - [src-tauri/src/solution.rs](src-tauri/src/solution.rs) - scalar/color tests
  - [src-tauri/src/plot3d.rs](src-tauri/src/plot3d.rs) - geometry/slicing tests (includes IBLANK, arbitrary planes, mesh invariants)

- **Testing Conventions:** [TESTING.md](TESTING.md) emphasizes unit tests for both TS and Rust

## Implementation Steps

### 1. Define Contour State & Contracts

**File:** [src/types/plot3d.ts](src/types/plot3d.ts)

Add new TypeScript types:
```typescript
export interface ContourSettings {
  enabled: boolean;
  level: number; // 0.0 to 1.0 normalized
}

export interface IsoSurfaceGeometry {
  gridId: string;
  positions: Float32Array;
  normals: Float32Array;
  indices: Uint32Array;
}

export interface ContourLineGeometry {
  sliceId: string;
  positions: Float32Array; // line segments as pairs of points
}
```

**File:** [src/App.tsx](src/App.tsx)

Add state variables:
```typescript
const [contoursEnabled, setContoursEnabled] = useState<boolean>(false);
const [contourLevel, setContourLevel] = useState<number>(0.5);
const [contourLevelWarning, setContourLevelWarning] = useState<string>("");
```

Add validation logic to clamp contour level and show warning when out of range.

### 2. Add Rust Tauri Commands

**File:** [src-tauri/src/lib.rs](src-tauri/src/lib.rs)

Add three new commands:

```rust
#[tauri::command]
async fn extract_iso_surface_by_id(
    grid_id: String,
    solution_id: String,
    scalar_field: ScalarField,
    level_normalized: f64,
    iblank_filter_mode: IblankFilterMode,
    respect_iblank: bool,
    show_fringe_points: bool,
) -> Result<IsoSurfaceGeometry, String> {
    // 1. Get cached grid and solution
    // 2. Compute actual level from normalized value × field range
    // 3. Call marching cubes kernel (to be implemented)
    // 4. Return triangulated mesh geometry
}

#[tauri::command]
async fn extract_slice_contours_by_id(
    grid_id: String,
    solution_id: String,
    slice_id: String,
    scalar_field: ScalarField,
    level_normalized: f64,
    iblank_filter_mode: IblankFilterMode,
    respect_iblank: bool,
    show_fringe_points: bool,
) -> Result<ContourLineGeometry, String> {
    // 1. Get cached grid and solution
    // 2. Get or recompute slice mesh with scalar values
    // 3. Compute actual level from normalized value × field range
    // 4. Call contour line extraction kernel (to be implemented)
    // 5. Return line segments
}

#[tauri::command]
async fn extract_arbitrary_plane_contours_by_id(
    grid_id: String,
    solution_id: String,
    plane_id: String,
    scalar_field: ScalarField,
    level_normalized: f64,
    iblank_filter_mode: IblankFilterMode,
    respect_iblank: bool,
    show_fringe_points: bool,
) -> Result<ContourLineGeometry, String> {
    // Similar to extract_slice_contours_by_id but for arbitrary planes
}
```

Register commands in builder:
```rust
tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
        // ... existing commands ...
        extract_iso_surface_by_id,
        extract_slice_contours_by_id,
        extract_arbitrary_plane_contours_by_id,
    ])
```

### 3. Implement Iso-Surface Extraction Kernel

**File:** [src-tauri/src/plot3d.rs](src-tauri/src/plot3d.rs)

Add marching cubes implementation:

```rust
impl Plot3DGrid {
    /// Extract iso-surface at given scalar field level using marching cubes
    pub fn extract_iso_surface(
        &self,
        solution: &Plot3DSolution,
        scalar_field: &ScalarField,
        level: f64,
        iblank_filter_mode: &IblankFilterMode,
        respect_iblank: bool,
        show_fringe_points: bool,
    ) -> Result<MeshGeometry, String> {
        // Implementation approach:
        // 1. Iterate over all structured grid cells (i,j,k)
        // 2. For each cell, compute scalar values at 8 corners
        // 3. Apply IBLANK filtering using existing cell inclusion rules
        // 4. Classify cell configuration vs level threshold
        // 5. Generate triangles at edge crossings (marching cubes lookup table)
        // 6. Interpolate positions and normals at crossing points
        // 7. Collect triangles into MeshGeometry
        // 8. Return geometry (positions, normals, indices)
    }
}
```

Key considerations:
- Reuse existing IBLANK filtering logic from slice/mesh generation
- Handle edge cases: degenerate cells, multi-grid scenarios, fringe points
- Generate consistent winding order for normals
- Consider performance: only process cells that could contain crossings

### 4. Implement Contour Line Extraction Kernel

**File:** [src-tauri/src/plot3d.rs](src-tauri/src/plot3d.rs)

Add contour line extraction over triangulated surfaces:

```rust
/// Extract contour lines at given level from triangulated mesh with scalar values
pub fn extract_contour_lines_from_mesh(
    mesh: &MeshGeometry,
    scalar_values: &[f64], // per-vertex scalar values
    level: f64,
) -> Result<Vec<[f64; 3]>, String> {
    // Implementation approach:
    // 1. Iterate over all triangles in mesh
    // 2. For each triangle, get scalar values at 3 vertices
    // 3. Classify edges against level threshold
    // 4. Generate line segments at crossing edges
    // 5. Interpolate positions along crossing edges
    // 6. Optionally weld segments into polylines
    // 7. Return line segment positions as pairs of points
}
```

Integration points:
- Call this from both `extract_slice_contours_by_id` and `extract_arbitrary_plane_contours_by_id`
- Reuse existing slice/arbitrary-plane mesh generation + scalar interpolation
- Leverage existing `VertexCellData` interpolation weights where applicable

### 5. Update Viewer3D Renderer

**File:** [src/components/Viewer3D.tsx](src/components/Viewer3D.tsx)

Add new render groups:

```typescript
// State for iso-surfaces (per grid)
const [isoSurfaceGeometries, setIsoSurfaceGeometries] = useState<Map<string, IsoSurfaceGeometry>>(new Map());

// State for contour lines (per slice/plane)
const [contourLineGeometries, setContourLineGeometries] = useState<ContourLineGeometry[]>([]);

// Material switching logic for slices
const sliceMaterial = contoursEnabled 
  ? mutedGreyMaterial  // new constant material
  : currentColoredMaterial; // existing field-colored material
```

Add recompute effects:
- Trigger iso-surface extraction when: `contoursEnabled`, `contourLevel`, `currentScalarField`, IBLANK settings change
- Trigger contour-line extraction when: `contoursEnabled`, `contourLevel`, `currentScalarField`, IBLANK settings change, slice/plane geometry changes
- Switch slice mesh material based on `contoursEnabled`

Render groups:
```typescript
// Iso-surfaces
{Array.from(isoSurfaceGeometries.values()).map(geom => (
  <mesh key={geom.gridId} geometry={...} material={solidColorMaterial} />
))}

// Contour lines
{contourLineGeometries.map(geom => (
  <lineSegments key={geom.sliceId} geometry={...} material={coloredLineMaterial} />
))}
```

Material definitions:
- `solidColorMaterial`: choose reasonable default color for iso-surfaces (e.g., semi-transparent blue/grey)
- `coloredLineMaterial`: choose contour line color (e.g., black or contrasting color)
- `mutedGreyMaterial`: muted grey for slice mesh background

### 6. Add UI Controls

**File:** [src/App.tsx](src/App.tsx) or [src/components/SolutionViewer.tsx](src/components/SolutionViewer.tsx)

Add controls in existing control panel region:

```typescript
<div className="contour-controls">
  <label>
    <input 
      type="checkbox" 
      checked={contoursEnabled}
      onChange={(e) => setContoursEnabled(e.target.checked)}
    />
    Enable Contours
  </label>
  
  <label>
    Contour Level (normalized):
    <input 
      type="number"
      min="0"
      max="1"
      step="0.01"
      value={contourLevel}
      onChange={(e) => {
        const value = parseFloat(e.target.value);
        if (value < 0 || value > 1) {
          const clamped = Math.max(0, Math.min(1, value));
          setContourLevel(clamped);
          setContourLevelWarning(`Level clamped to valid range [0, 1]`);
          setTimeout(() => setContourLevelWarning(""), 3000);
        } else {
          setContourLevel(value);
          setContourLevelWarning("");
        }
      }}
    />
  </label>
  
  {contourLevelWarning && (
    <div className="warning-message">{contourLevelWarning}</div>
  )}
</div>
```

Position near existing scalar field / color scheme controls for logical grouping.

### 7. Update Documentation

**Files to update:**
- [PLOT3D_COMMANDS.md](PLOT3D_COMMANDS.md): Add new command signatures and examples
- [ROADMAP.md](ROADMAP.md): Update status to reflect contour implementation, remove outdated notes

Add sections documenting:
- New Tauri commands with parameters and return types
- UI behavior and controls
- Contour level normalization and range handling
- IBLANK interaction
- Future enhancements (multi-level, iso-surface coloring by field)

## Testing & Verification

### Unit Tests - Rust

**File:** [src-tauri/src/plot3d.rs](src-tauri/src/plot3d.rs)

Add tests for iso-surface extraction:
```rust
#[test]
fn test_iso_surface_crossing() {
    // Create synthetic grid with known scalar gradient
    // Extract iso-surface at mid-level
    // Verify non-empty geometry returned
    // Verify triangle count in expected range
}

#[test]
fn test_iso_surface_outside_range() {
    // Extract iso-surface at level above/below all values
    // Verify empty geometry returned
}

#[test]
fn test_iso_surface_iblank_filtering() {
    // Create grid with IBLANK values
    // Extract with respect_iblank=true vs false
    // Verify geometry differs appropriately
}
```

Add tests for contour-line extraction:
```rust
#[test]
fn test_contour_lines_on_slice() {
    // Create synthetic slice mesh with known scalar field
    // Extract contour lines at known level
    // Verify line segments returned
    // Verify segment positions are correct
}

#[test]
fn test_contour_lines_iblank() {
    // Similar to iso-surface IBLANK test
}
```

### Unit Tests - TypeScript

**File:** [src/utils/solutionData.test.ts](src/utils/solutionData.test.ts) or new test file

Add tests for contour state validation:
```typescript
test('contour level clamping', () => {
  // Test that out-of-range values are clamped to [0, 1]
});

test('contour state initialization', () => {
  // Test default values
});
```

### Manual Regression Testing

Run application with `npm run tauri dev` and verify:

1. **Toggle behavior:**
   - Enable contours → slice mesh turns grey, contour lines appear
   - Disable contours → slice mesh returns to field coloring, contour lines disappear

2. **Level adjustment:**
   - Change level → geometry updates in real-time
   - Enter out-of-range value → clamped, warning displayed

3. **Field/range changes:**
   - Change scalar field → contours update using new field
   - Change IBLANK mode → contour geometry changes appropriately

4. **Multi-feature rendering:**
   - Enable both iso-surfaces and slice contours → both render simultaneously
   - Verify no z-fighting or visual conflicts

5. **Edge cases:**
   - Load multi-grid file → each grid gets separate iso-surface
   - Toggle slices on/off → contours appear/disappear with slices
   - Change arbitrary plane → contours update

### Regression Testing

Run existing test suites per [TESTING.md](TESTING.md):
```bash
# TypeScript tests
npm test

# Rust tests
cd src-tauri
cargo test
```

Verify all existing tests still pass (no regressions in slice/color/geometry logic).

## Future Enhancements

Beyond MVP scope, consider adding:

1. **Multi-level support:**
   - UI for adding/removing multiple contour levels
   - List editor for managing levels
   - Level set management

2. **Advanced iso-surface styling:**
   - Color iso-surfaces by a separate scalar field
   - Transparency controls
   - Per-grid color customization

3. **Contour line styling:**
   - Line width controls
   - Line color picker
   - Label contour values

4. **Performance optimizations:**
   - Adaptive decimation for large meshes
   - Level-of-detail for iso-surfaces
   - Incremental updates

5. **Auto-level generation:**
   - Evenly-spaced N levels across range
   - Logarithmic spacing option
   - Per-grid vs global range modes

6. **Export:**
   - Export contour geometry to standard formats
   - Export contour data values

## Known Issues & Blockers

### Documentation Drift
- [ROADMAP.md](ROADMAP.md) contains outdated status notes about arbitrary-plane coloring
- [PLOT3D_COMMANDS.md](PLOT3D_COMMANDS.md) command signatures don't match current implementation
- [ARBITRARY_PLANES.md](ARBITRARY_PLANES.md) lists color schemes not in current enums

**Resolution:** Update all documentation as part of Step 7.

### Architectural Considerations
- Current rendering is slice-centric (no slices → no render)
- Iso-surfaces should render even when slices are off
- May need to adjust Viewer3D render trigger logic

## Implementation Checklist

- [ ] Step 1: Define contour state & contracts in types + App.tsx
- [ ] Step 2: Add Rust Tauri commands in lib.rs
- [ ] Step 3: Implement marching cubes iso-surface kernel in plot3d.rs
- [ ] Step 4: Implement contour-line extraction kernel in plot3d.rs
- [ ] Step 5: Update Viewer3D renderer with new geometry groups
- [ ] Step 6: Add UI controls for toggle + level input
- [ ] Step 7: Update documentation (PLOT3D_COMMANDS.md, ROADMAP.md)
- [ ] Test: Rust unit tests for iso-surface extraction
- [ ] Test: Rust unit tests for contour-line extraction
- [ ] Test: TypeScript unit tests for state validation
- [ ] Test: Manual regression testing (all scenarios)
- [ ] Test: Run existing test suites (no regressions)

## References

- **Marching Cubes Algorithm:** Lorensen & Cline (1987)
- **Marching Triangles:** For 2D contour extraction on triangulated surfaces
- **Current Slice Implementation:** [src-tauri/src/plot3d.rs](src-tauri/src/plot3d.rs) `slice_grid`, `slice_arbitrary_plane_with_solution`
- **Color/Scalar Pipeline:** [src-tauri/src/solution.rs](src-tauri/src/solution.rs)
- **Testing Guidelines:** [TESTING.md](TESTING.md)
