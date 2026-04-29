# overview PLOT3D Viewer - Development Roadmap

## Overview
This document outlines the future development work needed to create a full-featured PLOT3D visualization application that replicates the capabilities of NASA's original PLOT3D software.

## Development Guidelines
**Always add unit tests when implementing new capabilities**
- Write tests alongside new features
- Cover both happy paths and error cases
- Test edge cases and invalid inputs
- Ensure test isolation for shared state
- Run `cargo test` before committing

## Troubleshooting & Lessons Learned

**Vertex color rendering (wireframe)**
- Line-based rendering can ignore vertex colors with the default `lineBasicMaterial`. We switched to a custom shader for `lineSegments` so the per-vertex `color` attribute is always used.
- When vertex colors are present, avoid tinting with a grid ID color; use a neutral base (white) or fully custom shader output.

**Color array integrity**
- The `colors` array must match the vertex array length (3 floats per vertex). If lengths mismatch, colors should be ignored or regenerated.
- Some datasets can produce non-finite scalar values (NaN/∞). These must be excluded from min/max and normalization to prevent white/invalid colors.

**Logging in Tauri**
- `console.log` isn’t visible in Tauri app logs. Use the shared logger so diagnostics show up in log export.
**IBLANK Filtering UI (March 2026)**
- IBLANK filtering is applied at mesh geometry generation, not at the grid cache level (cache is unchanged).
- When `Ignore IBLANK` is ON (`respectIblank=false`), the `Show Fringe Points` toggle is disabled (greyed out) — this prevents invalid flag combinations that would hide fringe points when blanking is ignored.
- Backend applies a safety normalization to prevent invalid flag combinations even if the frontend state diverges.
**Performance Optimizations (v0.2.1 - February 2026)**
- Automatic mesh decimation: Grids >250 nodes get 2-4x decimation for faster rendering
- Parallel normal computation using rayon reduces mesh generation time by ~50-70%
- Pre-allocated arrays eliminate reallocation overhead during mesh creation
- Frustum culling and bounding spheres skip rendering off-screen meshes
- Decimation thresholds: >1000 cells = 4x, >500 = 3x, >250 = 2x, ≤250 = full resolution

**Arbitrary Cutting Planes (v0.2.2 - February 2026)**
- Implemented marching cells algorithm for hexahedral grid-plane intersection
- Plane defined by point (3D coordinates) and normal vector (automatically normalized)
- Polygon extraction uses centroid-based angular sorting for proper winding order
- Edge-plane intersections use linear interpolation (parametric equations)
- Triangle fan tessellation from sorted polygon vertices
- Handles edge cases: zero normal, no intersection, degenerate cells
- UI provides numeric inputs for plane point (X,Y,Z) and normal (X,Y,Z)
- Future: Add solution field interpolation and interactive plane manipulation widgets
## Current Status ✅

**✨ Version: 0.2.0 (Multi-Grid Support) - In Progress**

### Core Capabilities Completed:
- [x] Tauri + React + TypeScript project setup
- [x] Three.js integration with React Three Fiber
- [x] Advanced 3D viewer with orbit controls & damping
- [x] Wireframe & shaded rendering modes
- [x] Complete PLOT3D file format support (binary & ASCII)
  - [x] Grid files (XYZ)
  - [x] Solution files (Q)
  - [x] Function files (F)
- [x] Multi-grid rendering with color coding
- [x] Grid visibility management & isolation
- [x] Comprehensive logging system (frontend + backend)
- [x] File dialog integration
- [x] Byte order & precision auto-detection
- [x] Unit test coverage (25+ tests)

## Phase 1: Core File I/O and Visualization

### 1.1 File Dialog Integration ✅
- [x] Add Tauri dialog plugin to `Cargo.toml`
- [x] Implement file picker UI for selecting PLOT3D files
- [x] Add support for multiple file selection (multi-grid)
- [x] Display file metadata (dimensions, number of grids)

### 1.2 PLOT3D Format Support ✅
- [x] Complete binary PLOT3D grid file reader
  - [x] Single-grid format
  - [x] Multi-grid format
  - [x] Both whole and plane formats
  - [x] Fortran unformatted record markers
  - [x] Auto-detect single (f32) vs double (f64) precision
  - [x] Combined XYZ records and separate XYZ records
- [x] Add ASCII PLOT3D grid file reader
- [x] Implement PLOT3D solution file reader (Q files)
  - [x] Binary format
  - [x] ASCII format
- [x] Add PLOT3D function file support
- [x] Handle byte order (big-endian vs little-endian)
- [x] Add file format validation and error handling

### 1.3 Logging System ✅
- [x] Implement structured logging (frontend & backend)
  - [x] Rust: Use `tracing` and `tracing-subscriber` for backend logging
  - [x] TypeScript: Use `pino` or `winston` for frontend logging
- [x] Add log levels (DEBUG, INFO, WARN, ERROR)
- [x] File operations logging (load, parse, errors)
- [x] User-visible log viewer panel in UI
- [x] Log persistence to disk
- [x] Log filtering and search
- [x] Integration with all Tauri commands
- [x] Error reporting to user with helpful messages
- [x] Performance event logging

### 1.4 Mesh Rendering ✅
- [x] Convert PLOT3D grid data to Three.js geometry
- [x] Render structured grid as wireframe
- [x] Implement surface extraction from 3D grids
- [x] Surface rendering with computed normals
- [x] **Performance optimizations**:
  - [x] Pre-allocation of vertex/index arrays
  - [x] Parallel normal computation with rayon
  - [x] Automatic mesh decimation (2-4x for large grids)
  - [x] Frustum culling with bounding sphere computation
- [ ] Add grid line rendering with customizable density
- [ ] Advanced optimizations for very large datasets:
  - [ ] Level of detail (LOD) system with dynamic switching
  - [ ] Instancing for repeated geometry
  - [x] Automatic mesh decimation for large grids
  - [x] Frustum culling enabled
- [ ] Add grid line rendering with customizable density
- [ ] Optimize for large meshes (million+ points)
  - [ ] Level of detail (LOD) system with distance-based switching
  - [ ] Instancing for repeated geometry

### 1.5 Multi-Grid Support ✅
- [x] Render multiple grids simultaneously
- [x] Color-code different grids
- [x] Toggle visibility per grid
- [x] Grid selection and isolation
- [x] Display grid hierarchy/tree structure

## Phase 2: Rendering Modes and Visualization

### 2.1 Rendering Modes
- [x] Wireframe mode
- [x] Flat shaded surfaces (with computed normals)
- [x] Smooth shaded surfaces (vertex normal averaging)
- [ ] Hidden line removal
- [x] Transparent surfaces (partial - dimming implemented)
- [ ] Point cloud rendering
- [x] Combination modes (wireframe + shaded)

### 2.2 Solution Data Visualization
- [x] Scalar field visualization
  - [x] Color mapping to scalar values
  - [x] Configurable color schemes (rainbow, grayscale, viridis, turbo, hot)
  - [x] Color bar/legend display with dynamic scheme selection
  - [x] Value range adjustment (min/max clipping)
    - Plan: `COLOR_MAP_MINMAX_CLIPPING_PLAN.md` (color-map-only scope, auto-correct invalid bounds, display actual range)
  - [x] Display scalar values via point probe (keypress-triggered sampling)
    - [x] Probe sampling only on `p` / `P` (no continuous hover updates)
    - [x] Snap mode (`P`) for nearest grid vertex
    - [x] Dedicated probe window output (non-overlapping placement)
    - [x] Report all derived function values (density, pressure, velocity, momentum, energy, gamma)
    - [x] Report sampled I/J/K index
- [ ] Vector field visualization
  - [ ] Arrow glyphs
  - [ ] Streamlines
  - [ ] Particle traces
- [x] Contour lines on surfaces (AUTOMATIC/INCREMENT/MANUAL/NONE modes; absolute physical values per ADR-0004)
  - [x] Contour level tick marks overlaid on ColorLegend bar (resolved absolute values, normalized to field range)
- [x] Iso-surfaces for 3D scalar fields (marching cubes; multi-level; opacity control)

### 2.3 Advanced Visualization Features
- [x] Cross-sectional slicing (I, J, K planes)
  - [x] Single and multi-slice per grid support
  - [x] Initial K=1 slice auto-created on first load
  - [x] Compact inline slice dropdown in grid list
  - [x] Per-slice controls: plane selection, index slider, 1-based display
  - [x] Add/remove slices dynamically
  - [x] Slice-only rendering (no full-grid fallback; no slices = no render)
  - [x] Slices persist across toggle (no auto-recreate on toggle)
- [x] Arbitrary cutting planes
  - [x] Plane defined by point and normal vector
  - [x] Marching cells algorithm for grid-plane intersection
  - [x] UI controls for plane positioning (point coordinates, normal vector)
  - [x] Integration with existing slice rendering pipeline
  - [x] Unit tests for plane intersection logic
  - [ ] Solution field coloring on arbitrary planes
  - [ ] Interactive plane manipulation (drag handles, rotation widgets)
- [ ] Volume rendering for 3D data
- [ ] Particle injection and flow visualization
- [ ] Texture mapping support

## Phase 3: Built-in Functions (74 Functions)

### 3.1 Function Categories
Research and implement PLOT3D's 74 built-in functions:
- [x] Document all 74 functions from PLOT3D manual
- [x] Categorize functions by type:
  - [ ] Coordinate transformations
  - [x] Flow variables (pressure, velocity, Mach number)
  - [x] Derivatives and gradients
  - [x] Thermodynamic properties
  - [x] Vorticity and turbulence metrics
  - [ ] Geometric calculations
- [x] Implement function evaluation system in Rust
- [x] Create UI for function selection and application
- [x] Grid-diagnostic functions catalogued (FUNCTION 0–99): walls, grids, IBLANK holes, orphan points, tet/crossing checks
- [x] Vector field functions catalogued (FUNCTION 200–299): velocity, vorticity, momentum, perturbation velocity, V×ω, pressure/density gradients
- [x] Particle-trace functions catalogued (FUNCTION 300–399): particle traces, vortex lines
- [x] Special overlay functions catalogued (FUNCTION 400+): shock locations by pressure gradient (plain + filtered)
- [ ] Vector field rendering (arrow glyphs / streamlines)
- [ ] Particle trace rendering
- [ ] Special overlay rendering (shock surfaces)
- [ ] Support custom user-defined functions

### 3.2 Data Processing
- [ ] Interpolation schemes (linear, cubic)
- [ ] Grid metrics calculation
- [ ] Coordinate system conversions
- [ ] Dimensionalization/non-dimensionalization

## Phase 4: User Interface and Interaction

### 4.1 UI Components
- [ ] File browser panel
- [ ] Grid/zone selection tree
- [ ] Rendering options panel
  - [ ] Mode selection
  - [ ] Lighting controls
  - [ ] Material properties
- [ ] Function calculator panel
- [ ] Color map editor
- [ ] View controls (save/restore camera positions)

### 4.2 Camera and Navigation
- [x] Orbit controls (basic)
- [x] Pan, zoom, rotate (via OrbitControls)
- [x] Damping for smooth interaction
- [ ] Preset views (front, back, left, right, top, bottom)
- [ ] Fit to view / reset camera
- [ ] Multiple viewport support
- [ ] Synchronized camera across viewports

### 4.3 Measurement and Analysis Tools
- [x] Point probe (display values at sampled cursor location)
  - [x] Interpolated mode (`p`)
  - [x] Snap-to-grid-point mode (`P`)
  - [x] World XYZ and I/J/K index reporting
- [ ] Distance measurement
- [ ] Area and volume calculations
- [ ] Line plot extraction
- [ ] Statistical summary of regions

## Phase 5: Animation and Export

### 5.1 Animation Support
- [ ] Time-series data loading
- [ ] Animation timeline/playback controls
- [ ] Frame interpolation
- [ ] Animation recording to video
- [ ] Keyframe-based camera animation

### 5.2 Export Capabilities
- [ ] Screenshot export (PNG, JPEG)
- [ ] High-resolution rendering
- [ ] 3D model export (OBJ, STL, GLTF)
- [ ] Data export (CSV, VTK)
- [ ] Scene/session save/load
- [ ] Export for animation software

## Phase 6: Performance and Optimization

### 6.1 Large Dataset Handling
- [x] **Automatic mesh decimation based on grid size** (2x, 3x, 4x reduction)
- [x] **Parallel processing** for normal computation (rayon)
- [x] **Pre-allocation** of vertex/index buffers
- [ ] Streaming/progressive loading for large files
- [ ] Memory-mapped file I/O
- [ ] GPU-accelerated computation (WebGPU)
- [ ] Adaptive resolution based on viewport

### 6.2 Rendering Optimization
- [x] **Frustum culling** enabled on all meshes
- [x] **Bounding sphere computation** for efficient culling
- [ ] Occlusion culling
- [ ] Geometry batching
- [ ] Shader optimization
- [ ] Web Workers for computation
- [ ] Wasm optimization flags

## Phase 7: Advanced Features

### 7.1 Comparative Visualization
- [ ] Side-by-side grid comparison
- [ ] Difference visualization
- [ ] Overlay multiple solutions

### 7.2 Scripting and Automation
- [ ] Command-line interface
- [ ] Batch processing mode
- [ ] Python/JavaScript scripting API
- [ ] Macro recording and playback

### 7.3 Collaboration Features
- [ ] Session sharing
- [ ] Annotations and markup
- [ ] Export presentations/reports

## Phase 8: Testing and Documentation

### 8.1 Testing
- [x] Unit tests for PLOT3D parser (30+ tests in plot3d.rs including metadata parsing)
- [x] Unit tests for grid utilities (TypeScript)
- [x] Unit tests for solution data computation (Rust + TypeScript)
- [x] Test framework setup (Vitest + Rust test harness)
- [ ] Integration tests for file I/O
- [x] Visual regression tests for rendering (headless PNG exporter with SHA-256 + semantic drift checks; contour AUTOMATIC/INCREMENT/MANUAL fixture coverage)
- [ ] Performance benchmarks
- [ ] Test with real CFD datasets (larger variety needed)
- [ ] Cross-platform testing (Linux, Windows, macOS)

### 8.2 Documentation
- [ ] User manual
- [ ] API documentation
- [ ] Tutorial videos
- [ ] Example datasets
- [ ] PLOT3D format specification reference
- [ ] Developer contribution guide

## 🎯 Immediate Next Steps (Priority Order)

### High Priority - Core Visualization Enhancements
1. **Solution Data Visualization** (Phase 2.2) - MOSTLY COMPLETE ✅
   - [x] Gamma extraction from solution files (Q[5])
   - [x] Pressure calculation using gamma
   - [x] Scalar field color mapping (density, pressure, velocity, etc.)
   - [x] Configurable color schemes (rainbow, grayscale, viridis, turbo, hot)
   - [x] Metadata parsing from PLOT3D solution files
   - [x] Color bar/legend UI component with dynamic scheme selection
   - [x] Allow value range adjustment (min/max clipping)
   - [ ] Display scalar values on hover (point probe)

2. **Rendering Improvements** (Phase 2.1)
   - Implement smooth shading (average vertex normals)
   - Add transparency controls for overlapping grids
   - Implement point cloud rendering mode
   - Add combination rendering (wireframe overlay on shaded)

3. **Camera Presets & Navigation** (Phase 4.2)
   - Add preset camera views (XY, XZ, YZ planes)
   - Implement "fit to view" / auto-zoom
   - Add camera reset button
   - Save/restore camera positions

### Medium Priority - Performance & Usability
4. **Large Dataset Optimization** (Phase 6.1)
   - Implement progressive loading for large files
   - Add geometry simplification/decimation
   - Profile memory usage and optimize allocations
   - Test with meshes >1M vertices

5. **UI/UX Enhancements** (Phase 4.1)
   - Improve grid tree UI (search/filter grids)
   - Add grid statistics panel (bounds, cell count, quality metrics)
   - Implement keyboard shortcuts
   - Add dark/light theme toggle
   - Drag-and-drop file loading

6. **Cross-Sectional Slicing** (Phase 2.3)
   - Extract I/J/K plane slices
   - Render cutting planes
   - Interactive plane positioning

### Low Priority - Advanced Features  
7. **Function Support** (Phase 3)
   - Research PLOT3D's 74 built-in functions
   - Implement basic derived variables (velocity magnitude, pressure)
   - Create function calculator UI

8. **Export & Sharing** (Phase 5.2)
   - Screenshot export (high-res PNG)
   - 3D model export (OBJ/STL)
   - Session save/load (remember loaded files & settings)

9. **Command Files & Batch Processing** (Phase 5.3)
   - Record interactive operations as command files
   - Load and replay command files
   - Batch processing mode (CLI: `overview --batch command_file.plot3d`)
   - Command file format with comments, loops, conditionals
   - Enable reproducible workflows and automation

## Technical Debt and Refactoring

### Code Quality
- [x] Add comprehensive error handling (Rust Result types, error logging)
- [x] Implement proper TypeScript types (typed interfaces for grids, solutions)
- [x] Add logging system (see Phase 1.3)
- [x] File validation and error messages
- [ ] Performance profiling and monitoring
- [ ] Security audit for file handling

### Architecture
- [ ] State management (Redux/Zustand)
- [ ] Modular plugin system for functions
- [ ] Settings/preferences persistence
- [ ] Undo/redo system
- [ ] Event system for component communication

## Research Items

### Format Compatibility
- [ ] Research PLOT3D format variants in the wild
- [ ] Support other CFD formats (VTK, HDF5, CGNS, NetCDF)
- [ ] Investigate PLOT3D extensions and vendor-specific formats

### Visualization Techniques
- [ ] Study modern scientific visualization best practices
- [ ] Evaluate WebGPU for next-gen rendering
- [ ] Research volume rendering algorithms
- [ ] Investigate GPU-based particle systems

## Dependencies to Add

### Frontend
- [ ] `@tauri-apps/plugin-dialog` - File dialogs
- [ ] `zustand` or `redux` - State management
- [ ] `@mantine/core` or `shadcn-ui` - UI components
- [ ] `react-colorful` - Color picker for maps
- [ ] `leva` - Debug GUI controls

### Backend (Rust)
- [ ] `byteorder` - Byte order handling
- [ ] `memmap2` - Memory-mapped files
- [ ] `rayon` - Parallel processing
- [ ] `thiserror` - Better error types
- [ ] `tracing` - Structured logging

## Release Milestones

### v0.1.0 - MVP (Minimum Viable Product)
- Basic file loading and grid visualization
- Wireframe and shaded modes
- Single grid support

### v0.2.0 - Multi-Grid Support
- Multiple grid rendering
- Grid management UI
- Improved performance

### v0.3.0 - Solution Visualization
- Solution file support
- Scalar field color mapping
- Basic function support

### v0.4.0 - Advanced Visualization
- Cross-sections and slicing
- Vector fields
- Animation support

### v1.0.0 - Full PLOT3D Parity
- All 74 built-in functions
- Complete format support
- Professional UI
- Full documentation

## Notes
- Prioritize features based on user feedback
- Maintain cross-platform compatibility throughout
- Regular performance testing with large datasets
- Keep UI responsive even with heavy computation

## 📊 Development Progress Summary

### What's Working Well:
- **Robust File Parsing**: Successfully handles various PLOT3D formats with auto-detection
- **Multi-Grid Architecture**: Clean separation between file groups and individual grids
- **Logging Infrastructure**: Comprehensive dual-channel logging (Rust + TypeScript)
- **Type Safety**: Strong typing throughout the stack reduces runtime errors
- **Test Coverage**: Good foundation with 25+ Rust tests and frontend utilities tested

### Known Limitations:
- **No Solution Visualization**: Can load Q files but not yet visualized
- **Performance**: Not yet optimized for very large meshes (>1M vertices)
- **Camera Controls**: Basic but lacks presets and advanced features
- **No Slicing/Cutting**: Can't yet view internal structures
- **Limited Rendering Modes**: Only wireframe and basic shading

### Recommended Development Focus:
1. **Solution visualization** - This is the biggest gap between current state and useful CFD tool
2. **Camera presets** - Quick wins for usability
3. **Performance optimization** - Critical before handling real-world datasets
4. **Slicing/cutting planes** - Essential for 3D data exploration

### Architecture Decisions to Consider:
- **State Management**: Currently using React useState; may need Zustand/Redux as complexity grows
- **WebGPU**: Consider for next-gen performance (Three.js r152+ has WebGPU support)
- **Worker Threads**: Offload mesh generation to Web Workers for UI responsiveness
- **Caching Strategy**: Cache converted meshes to avoid repeated computation
