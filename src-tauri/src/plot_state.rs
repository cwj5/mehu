/// Shared plot state, actions, and diagnostics model.
///
/// `PlotState` is the single source of truth for all visualization-critical
/// configuration, regardless of whether that configuration arrived via a parsed
/// `.com` script or a GUI interaction.  GUI interactions MUST commit to this
/// state (via `apply_action`) on apply/release — not on every drag frame.
///
/// The state transition function `apply_action` is pure: it takes the current
/// `PlotState` and a `PlotAction`, returns a new `PlotState` and a (possibly
/// empty) `Vec<Diagnostic>`.  This makes it easy to test without Tauri
/// plumbing.
use serde::{Deserialize, Serialize};

// ──────────────────────────────────────────────────────────────────────────────
// Capability IDs (kept in sync with capability_catalog.md)
// ──────────────────────────────────────────────────────────────────────────────

/// Canonical capability identifiers.  These strings are the same ones used in
/// `parity_matrix.json`; keep them in sync.
pub mod cap {
    pub const READ: &str = "READ";
    pub const FUNCTION: &str = "FUNCTION";
    pub const VIEW: &str = "VIEW";
    pub const VPOINT: &str = "VPOINT";
    pub const MINMAX: &str = "MINMAX";
    pub const CONTOURS: &str = "CONTOURS";
    pub const PLOT: &str = "PLOT";
    pub const WALLS: &str = "WALLS";
    pub const SUBSETS: &str = "SUBSETS";
    pub const FSURFACE: &str = "FSURFACE";
    pub const TEXT: &str = "TEXT";
    pub const SHOW: &str = "SHOW";
}

// ──────────────────────────────────────────────────────────────────────────────
// Scalar field
// ──────────────────────────────────────────────────────────────────────────────

/// The canonical scalar-field enum.  The frontend `ScalarField` type in
/// `src/utils/solutionData.ts` mirrors these variants exactly.  Legacy
/// `FUNCTION` numbers are a translation-layer concern and must not leak into
/// this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalarField {
    None,
    Density,
    UVelocity,
    VVelocity,
    WVelocity,
    VelocityMagnitude,
    MomentumX,
    MomentumY,
    MomentumZ,
    Pressure,
    Energy,

    // Legacy FUNCTION placeholders (known IDs, equations pending implementation)
    NormalizedDensity,
    StagnationDensity,
    NormalizedStagnationDensity,
    LogNormalizedDensity,
    NormalizedPressure,
    StagnationPressure,
    NormalizedStagnationPressure,
    PressureCoefficient,
    StagnationPressureCoefficient,
    PitotPressure,
    PitotPressureRatio,
    DynamicPressure,
    LogNormalizedPressure,
    Temperature,
    NormalizedTemperature,
    StagnationTemperature,
    NormalizedStagnationTemperature,
    LogNormalizedTemperature,
    Enthalpy,
    NormalizedEnthalpy,
    StagnationEnthalpy,
    NormalizedStagnationEnthalpy,
    InternalEnergy,
    NormalizedInternalEnergy,
    StagnationEnergy,
    NormalizedStagnationEnergy,
    KineticEnergy,
    NormalizedKineticEnergy,
    MachNumber,
    SpeedOfSound,
    CrossFlowVelocity,
    Normalized2dStreamFunction,
    VelocityDivergence,
    Entropy,
    EntropyMeasureS1,
    VorticityX,
    VorticityY,
    VorticityZ,
    VorticityMagnitude,
    Swirl,
    VelocityCrossVorticityMagnitude,
    HelicityDensity,
    RelativeHelicity,
    FilteredRelativeHelicity,
    ShockFunctionPressureGradient,
    FilteredShockFunction,
    PressureGradientMagnitude,
    DensityGradientMagnitude,
}

impl Default for ScalarField {
    fn default() -> Self {
        ScalarField::None
    }
}

impl ScalarField {
    /// Parse a lowercase snake_case string into a `ScalarField` variant.
    /// Returns `None` for unknown strings; `ScalarField::None` is not a valid
    /// string form — it is the default/unset state, not a named field.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "density" => Some(ScalarField::Density),
            "u_velocity" => Some(ScalarField::UVelocity),
            "v_velocity" => Some(ScalarField::VVelocity),
            "w_velocity" => Some(ScalarField::WVelocity),
            "velocity_magnitude" => Some(ScalarField::VelocityMagnitude),
            "momentum_x" => Some(ScalarField::MomentumX),
            "momentum_y" => Some(ScalarField::MomentumY),
            "momentum_z" => Some(ScalarField::MomentumZ),
            "pressure" => Some(ScalarField::Pressure),
            "energy" => Some(ScalarField::Energy),
            _ => None,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Contour model  (absolute physical values, multi-level)
// ──────────────────────────────────────────────────────────────────────────────

/// A single contour surface/line entry at a specific absolute field value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContourEntry {
    /// Absolute physical value of the contour (NOT normalized 0..1).
    pub value: f64,
    /// Optional RGBA color override for this level (r,g,b,a each in 0..=1).
    pub color: Option<[f32; 4]>,
}

/// How contour levels are specified.  Automatic and increment-based specs are
/// resolved to explicit entries before rendering, but the original intent is
/// preserved so the GUI can display a compact form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ContourSpec {
    /// No contours displayed.
    None,
    /// Renderer picks N evenly spaced absolute levels across the global field
    /// range.  `count` must be ≥ 1.
    Automatic { count: u32 },
    /// Contours at every `increment` physical units starting from `start`.
    Increment { start: f64, increment: f64 },
    /// Explicit list of contour entries (absolute values, optional colors).
    Manual { entries: Vec<ContourEntry> },
}

impl Default for ContourSpec {
    fn default() -> Self {
        ContourSpec::None
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// View / camera
// ──────────────────────────────────────────────────────────────────────────────

/// Which standard axis view is active (maps to a camera preset).
///
/// `rename_all = "snake_case"` handles simple cases like `PlusX → "plus_x"`.
/// The multi-letter plane variants (`PlaneXY`, etc.) each need an explicit
/// rename because serde would otherwise produce `"plane_x_y"` instead of the
/// `"plane_xy"` string used by the frontend `AXIS_VIEW_OPTIONS` constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AxisView {
    /// +X looking toward –X (right-hand side view).
    PlusX,
    MinusX,
    PlusY,
    MinusY,
    PlusZ,
    MinusZ,
    /// Two-axis plane views for 2D and function-surface (carpet) plots.
    /// XY = TOP, XZ = SIDE, YZ = FRONT in legacy PLOT3D terminology.
    #[serde(rename = "plane_xy")]
    PlaneXY,
    #[serde(rename = "plane_xz")]
    PlaneXZ,
    #[serde(rename = "plane_yz")]
    PlaneYZ,
    #[serde(rename = "plane_yx")]
    PlaneYX,
    #[serde(rename = "plane_zx")]
    PlaneZX,
    #[serde(rename = "plane_zy")]
    PlaneZY,
    /// No axis-aligned preset; an explicit viewpoint is used instead.
    Custom,
}

impl Default for AxisView {
    fn default() -> Self {
        AxisView::Custom
    }
}

/// Camera viewpoint specified by a 3D point.  This is the position the camera
/// looks *from* (legacy `VPOINT` semantics).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewPoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

// ──────────────────────────────────────────────────────────────────────────────
// MINMAX overrides
// ──────────────────────────────────────────────────────────────────────────────

/// Inclusive axis bounds; `min` must be strictly less than `max`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AxisBounds {
    pub min: f64,
    pub max: f64,
}

/// Per-axis plot range overrides from the `MINMAX` command.
///
/// Each spatial axis is independently optional; `None` means use the data
/// range for that axis.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct MinMaxOverride {
    pub x: Option<AxisBounds>,
    pub y: Option<AxisBounds>,
    pub z: Option<AxisBounds>,
}

// ──────────────────────────────────────────────────────────────────────────────
// WALLS / SUBSETS
// ──────────────────────────────────────────────────────────────────────────────

/// A named selection of grid index ranges used by `WALLS` and `SUBSETS`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexRange {
    /// 1-based inclusive start index. Negative values count from the end: -1 = last index.
    pub start: i32,
    /// 1-based inclusive end index; `None` means "to the end". Negative values count from the end.
    pub end: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GridSubset {
    /// 1-based grid number this subset applies to.
    pub grid: u32,
    #[serde(default)]
    pub gui_managed: bool,
    pub i_range: Option<IndexRange>,
    pub j_range: Option<IndexRange>,
    pub k_range: Option<IndexRange>,
}

// ──────────────────────────────────────────────────────────────────────────────
// FSURFACE
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FsurfaceSpec {
    /// Absolute scalar value at which the iso-surface is drawn.
    pub value: f64,
    pub scalar_field: ScalarField,
}

// ──────────────────────────────────────────────────────────────────────────────
// TEXT
// ──────────────────────────────────────────────────────────────────────────────

/// Annotation text to overlay on the plot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlotText {
    pub content: String,
    /// Normalized [0,1] viewport X position.
    pub x: f64,
    /// Normalized [0,1] viewport Y position.
    pub y: f64,
}

// ──────────────────────────────────────────────────────────────────────────────
// Plot mode (PLOT)
// ──────────────────────────────────────────────────────────────────────────────

/// Modern plot mode representation for legacy `PLOT` behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlotMode {
    Surface3d,
    Contours,
    Lines,
}

impl Default for PlotMode {
    fn default() -> Self {
        PlotMode::Surface3d
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Dataset references
// ──────────────────────────────────────────────────────────────────────────────

/// References to the currently active grid and solution files.  Cache IDs are
/// the string keys assigned by the Tauri caching layer (`load_plot3d_*_cached`).
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatasetRef {
    /// The cache ID of the active grid, if one has been loaded.
    pub grid_id: Option<String>,
    /// The cache ID of the active solution, if one has been loaded.
    pub solution_id: Option<String>,
}

// ──────────────────────────────────────────────────────────────────────────────
// PlotState — the single source of truth
// ──────────────────────────────────────────────────────────────────────────────

/// All visualization-critical configuration.  This struct is serialized and
/// sent to the frontend in response to `get_plot_state` for dev inspection.
///
/// Fields correspond 1:1 to in-scope capabilities in `capability_catalog.md`.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlotState {
    // READ
    pub dataset: DatasetRef,

    // FUNCTION
    pub scalar_field: ScalarField,

    // VIEW
    pub axis_view: AxisView,

    // VPOINT
    pub viewpoint: Option<ViewPoint>,

    // MINMAX
    pub minmax: MinMaxOverride,

    // CONTOURS
    pub contour_spec: ContourSpec,

    // WALLS
    pub walls: Vec<GridSubset>,

    // SUBSETS
    pub subsets: Vec<GridSubset>,

    // FSURFACE
    pub fsurface: Option<FsurfaceSpec>,

    // TEXT
    pub text_annotations: Vec<PlotText>,

    // PLOT
    pub plot_mode: PlotMode,
}

// ──────────────────────────────────────────────────────────────────────────────
// PlotAction — one variant per supported capability
// ──────────────────────────────────────────────────────────────────────────────

/// A typed, capability-scoped state mutation.  Both the parser executor and
/// GUI widgets produce `PlotAction` values and submit them through
/// `apply_action`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlotAction {
    // READ: set (or clear) the active dataset references.
    SetDataset(DatasetRef),

    // FUNCTION: choose which scalar field to visualise.
    SetScalarField(ScalarField),

    // VIEW: select a named axis-aligned camera preset.
    SetAxisView(AxisView),

    // VPOINT: set an explicit camera look-from point.
    SetViewpoint(ViewPoint),

    // MINMAX: override the color-map scalar range.
    SetMinMax(MinMaxOverride),

    // CONTOURS: replace the contour specification.
    SetContourSpec(ContourSpec),

    // WALLS: replace the complete walls list.
    SetWalls(Vec<GridSubset>),

    // WALLS/ADD: append entries to the existing walls list.
    AddWalls(Vec<GridSubset>),

    // SUBSETS: replace the complete subsets list.
    SetSubsets(Vec<GridSubset>),

    // SUBSETS/ADD: append entries to the existing subsets list.
    AddSubsets(Vec<GridSubset>),

    // FSURFACE: set or clear the iso-surface spec.
    SetFsurface(Option<FsurfaceSpec>),

    // TEXT: append a text annotation.
    AddTextAnnotation(PlotText),

    // TEXT: clear all text annotations.
    ClearTextAnnotations,

    // SHOW: emit a status snapshot in executor output.
    ShowStatus,

    // PLOT: set rendering mode (surface, contour, line).
    SetPlotMode(PlotMode),

    // PLOT: commit current state as a render intent (handled by the executor
    // layer; `apply_action` records the intent but does not render).
    CommitPlot,
}

// ──────────────────────────────────────────────────────────────────────────────
// Diagnostics
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

/// A diagnostic emitted during state transition or script execution.  Design
/// matches the source-location fields needed by a future parser integration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Capability ID (`cap::*`) that produced this diagnostic.
    pub capability: String,
    pub severity: DiagnosticSeverity,
    /// Source file that generated this diagnostic, if applicable.
    pub file: Option<String>,
    /// 1-based line number in the source file, if applicable.
    pub line: Option<u32>,
    /// 1-based column in the source file, if applicable.
    pub column: Option<u32>,
    pub message: String,
}

impl Diagnostic {
    pub fn warning(capability: &str, message: impl Into<String>) -> Self {
        Diagnostic {
            capability: capability.to_owned(),
            severity: DiagnosticSeverity::Warning,
            file: None,
            line: None,
            column: None,
            message: message.into(),
        }
    }

    pub fn error(capability: &str, message: impl Into<String>) -> Self {
        Diagnostic {
            capability: capability.to_owned(),
            severity: DiagnosticSeverity::Error,
            file: None,
            line: None,
            column: None,
            message: message.into(),
        }
    }

    pub fn info(capability: &str, message: impl Into<String>) -> Self {
        Diagnostic {
            capability: capability.to_owned(),
            severity: DiagnosticSeverity::Info,
            file: None,
            line: None,
            column: None,
            message: message.into(),
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tauri command return type
// ──────────────────────────────────────────────────────────────────────────────

/// Named return value for the `apply_plot_action` Tauri command.
/// Using a struct rather than a bare tuple gives the frontend clearly-named
/// fields (`result.state`, `result.diagnostics`) and keeps the API stable if
/// we need to add fields later.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyActionResult {
    pub state: PlotState,
    pub diagnostics: Vec<Diagnostic>,
}

// ──────────────────────────────────────────────────────────────────────────────
// State transition
// ──────────────────────────────────────────────────────────────────────────────

/// Apply a single `PlotAction` to `state`, returning the updated state and any
/// diagnostics.  This function is pure: it does not touch any global cache,
/// Tauri state, or I/O.
pub fn apply_action(mut state: PlotState, action: PlotAction) -> (PlotState, Vec<Diagnostic>) {
    let mut diags: Vec<Diagnostic> = Vec::new();

    const DEFAULT_VIEW_DISTANCE: f64 = 8.660_254_037_844_387;

    let view_distance_from = |vp: Option<&ViewPoint>| {
        vp.map(|v| (v.x * v.x + v.y * v.y + v.z * v.z).sqrt())
            .filter(|d| d.is_finite() && *d > 1e-6)
            .unwrap_or(DEFAULT_VIEW_DISTANCE)
    };

    let axis_viewpoint = |view: AxisView, distance: f64| -> Option<ViewPoint> {
        match view {
            AxisView::PlusX => Some(ViewPoint {
                x: distance,
                y: 0.0,
                z: 0.0,
            }),
            AxisView::MinusX => Some(ViewPoint {
                x: -distance,
                y: 0.0,
                z: 0.0,
            }),
            AxisView::PlusY => Some(ViewPoint {
                x: 0.0,
                y: distance,
                z: 0.0,
            }),
            AxisView::MinusY => Some(ViewPoint {
                x: 0.0,
                y: -distance,
                z: 0.0,
            }),
            AxisView::PlusZ => Some(ViewPoint {
                x: 0.0,
                y: 0.0,
                z: distance,
            }),
            AxisView::MinusZ => Some(ViewPoint {
                x: 0.0,
                y: 0.0,
                z: -distance,
            }),
            // Plane aliases map to orthogonal axis views.
            AxisView::PlaneXY | AxisView::PlaneYX => Some(ViewPoint {
                x: 0.0,
                y: 0.0,
                z: distance,
            }),
            AxisView::PlaneXZ | AxisView::PlaneZX => Some(ViewPoint {
                x: 0.0,
                y: distance,
                z: 0.0,
            }),
            AxisView::PlaneYZ | AxisView::PlaneZY => Some(ViewPoint {
                x: distance,
                y: 0.0,
                z: 0.0,
            }),
            AxisView::Custom => None,
        }
    };

    match action {
        PlotAction::SetDataset(dataset) => {
            state.dataset = dataset;
        }

        PlotAction::SetScalarField(field) => {
            state.scalar_field = field;
        }

        PlotAction::SetAxisView(view) => {
            let distance = view_distance_from(state.viewpoint.as_ref());
            state.axis_view = view;
            if let Some(vp) = axis_viewpoint(view, distance) {
                state.viewpoint = Some(vp);
            }
        }

        PlotAction::SetViewpoint(vp) => {
            state.viewpoint = Some(vp);
            // Explicit viewpoint supersedes a named axis preset.
            state.axis_view = AxisView::Custom;
        }

        PlotAction::SetMinMax(mm) => {
            let mut apply_axis =
                |label: &str, bounds: Option<AxisBounds>, dest: &mut Option<AxisBounds>| {
                    if let Some(ref b) = bounds {
                        if b.min >= b.max {
                            diags.push(Diagnostic::warning(
                                cap::MINMAX,
                                format!(
                                "MINMAX {label}-axis min ({}) must be less than max ({}); ignored",
                                b.min, b.max
                            ),
                            ));
                            return;
                        }
                    }
                    *dest = bounds;
                };
            apply_axis("x", mm.x, &mut state.minmax.x);
            apply_axis("y", mm.y, &mut state.minmax.y);
            apply_axis("z", mm.z, &mut state.minmax.z);
        }

        PlotAction::SetContourSpec(spec) => {
            if let ContourSpec::Automatic { count } = &spec {
                if *count == 0 {
                    diags.push(Diagnostic::warning(
                        cap::CONTOURS,
                        "Automatic contour count must be ≥ 1; defaulting to 10",
                    ));
                    state.contour_spec = ContourSpec::Automatic { count: 10 };
                } else {
                    state.contour_spec = spec;
                }
            } else if let ContourSpec::Increment { increment, .. } = &spec {
                if *increment <= 0.0 {
                    diags.push(Diagnostic::warning(
                        cap::CONTOURS,
                        "Contour increment must be > 0; ignored",
                    ));
                    // Leave state unchanged.
                } else {
                    state.contour_spec = spec;
                }
            } else {
                state.contour_spec = spec;
            }
        }

        PlotAction::SetWalls(walls) => {
            state.walls = walls;
        }

        PlotAction::AddWalls(mut walls) => {
            state.walls.append(&mut walls);
        }

        PlotAction::SetSubsets(subsets) => {
            state.subsets = subsets;
        }

        PlotAction::AddSubsets(mut subsets) => {
            state.subsets.append(&mut subsets);
        }

        PlotAction::SetFsurface(fs) => {
            state.fsurface = fs;
        }

        PlotAction::AddTextAnnotation(text) => {
            state.text_annotations.push(text);
        }

        PlotAction::ClearTextAnnotations => {
            state.text_annotations.clear();
        }

        PlotAction::ShowStatus => {
            // SHOW does not mutate PlotState; executor owns output formatting.
            diags.push(Diagnostic::info(cap::SHOW, "Show status requested"));
        }

        PlotAction::SetPlotMode(mode) => {
            state.plot_mode = mode;
        }

        PlotAction::CommitPlot => {
            // CommitPlot itself does not mutate state; the executor layer is
            // responsible for deriving a RenderIntent from the current state
            // when it handles this action.  We emit an info diagnostic so
            // callers can observe the boundary in a diagnostic stream.
            diags.push(Diagnostic::info(cap::PLOT, "Plot committed"));
        }
    }

    (state, diags)
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn default_state() -> PlotState {
        PlotState::default()
    }

    // ── SetDataset ────────────────────────────────────────────────────────────

    #[test]
    fn set_dataset_stores_ids() {
        let state = default_state();
        let action = PlotAction::SetDataset(DatasetRef {
            grid_id: Some("grid-1".to_owned()),
            solution_id: Some("sol-1".to_owned()),
        });
        let (new_state, diags) = apply_action(state, action);
        assert_eq!(new_state.dataset.grid_id.as_deref(), Some("grid-1"));
        assert_eq!(new_state.dataset.solution_id.as_deref(), Some("sol-1"));
        assert!(diags.is_empty());
    }

    #[test]
    fn set_dataset_clears_solution_when_none() {
        let mut state = default_state();
        state.dataset.solution_id = Some("old-sol".to_owned());
        let action = PlotAction::SetDataset(DatasetRef {
            grid_id: Some("grid-1".to_owned()),
            solution_id: None,
        });
        let (new_state, diags) = apply_action(state, action);
        assert!(new_state.dataset.solution_id.is_none());
        assert!(diags.is_empty());
    }

    // ── SetScalarField ────────────────────────────────────────────────────────

    #[test]
    fn set_scalar_field_updates_field() {
        let state = default_state();
        let (new_state, diags) =
            apply_action(state, PlotAction::SetScalarField(ScalarField::Pressure));
        assert_eq!(new_state.scalar_field, ScalarField::Pressure);
        assert!(diags.is_empty());
    }

    // ── SetAxisView / SetViewpoint interaction ────────────────────────────────

    #[test]
    fn set_axis_view_sets_axis_aligned_viewpoint() {
        let mut state = default_state();
        state.viewpoint = Some(ViewPoint {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        });
        let (new_state, diags) = apply_action(state, PlotAction::SetAxisView(AxisView::PlusZ));
        assert_eq!(new_state.axis_view, AxisView::PlusZ);
        let vp = new_state.viewpoint.expect("viewpoint should be set");
        assert_eq!(vp.x, 0.0);
        assert_eq!(vp.y, 0.0);
        assert!(vp.z > 0.0);
        assert!(diags.is_empty());
    }

    #[test]
    fn set_axis_view_sets_default_viewpoint_when_none() {
        let state = default_state();
        let (new_state, diags) = apply_action(state, PlotAction::SetAxisView(AxisView::PlaneXY));
        assert_eq!(new_state.axis_view, AxisView::PlaneXY);
        assert_eq!(
            new_state.viewpoint,
            Some(ViewPoint {
                x: 0.0,
                y: 0.0,
                z: 8.660_254_037_844_387,
            })
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn set_viewpoint_sets_custom_axis_view() {
        let state = default_state();
        let vp = ViewPoint {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        let (new_state, diags) = apply_action(state, PlotAction::SetViewpoint(vp.clone()));
        assert_eq!(new_state.axis_view, AxisView::Custom);
        assert_eq!(new_state.viewpoint, Some(vp));
        assert!(diags.is_empty());
    }

    // ── AxisView serde round-trip ─────────────────────────────────────────────

    #[test]
    fn axis_view_serde_round_trip_plane_variants() {
        // Confirm every plane alias serialises to the snake_case string the
        // frontend `AXIS_VIEW_OPTIONS` values use and can be deserialised back.
        let cases = [
            (AxisView::PlaneXY, "\"plane_xy\""),
            (AxisView::PlaneXZ, "\"plane_xz\""),
            (AxisView::PlaneYZ, "\"plane_yz\""),
            (AxisView::PlaneYX, "\"plane_yx\""),
            (AxisView::PlaneZX, "\"plane_zx\""),
            (AxisView::PlaneZY, "\"plane_zy\""),
            (AxisView::PlusX, "\"plus_x\""),
            (AxisView::Custom, "\"custom\""),
        ];
        for (variant, expected_json) in &cases {
            let serialised = serde_json::to_string(variant)
                .unwrap_or_else(|e| panic!("serialize {:?}: {}", variant, e));
            assert_eq!(
                serialised, *expected_json,
                "Serialisation mismatch for {:?}",
                variant
            );
            let round_tripped: AxisView = serde_json::from_str(*expected_json)
                .unwrap_or_else(|e| panic!("deserialize {}: {}", expected_json, e));
            assert_eq!(
                round_tripped, *variant,
                "Round-trip mismatch for {:?}",
                variant
            );
        }
    }

    // ── SetMinMax ─────────────────────────────────────────────────────────────

    #[test]
    fn set_minmax_accepts_valid_range() {
        let state = default_state();
        let mm = MinMaxOverride {
            x: Some(AxisBounds { min: 0.5, max: 2.5 }),
            y: None,
            z: None,
        };
        let (new_state, diags) = apply_action(state, PlotAction::SetMinMax(mm.clone()));
        assert_eq!(new_state.minmax, mm);
        assert!(diags.is_empty());
    }

    #[test]
    fn set_minmax_rejects_inverted_range() {
        let state = default_state();
        let mm = MinMaxOverride {
            x: Some(AxisBounds { min: 5.0, max: 1.0 }),
            y: None,
            z: None,
        };
        let (new_state, diags) = apply_action(state, PlotAction::SetMinMax(mm));
        // State must be unchanged.
        assert_eq!(new_state.minmax, MinMaxOverride::default());
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, DiagnosticSeverity::Warning);
        assert_eq!(diags[0].capability, cap::MINMAX);
    }

    #[test]
    fn set_minmax_rejects_equal_min_max() {
        let state = default_state();
        let mm = MinMaxOverride {
            x: Some(AxisBounds { min: 3.0, max: 3.0 }),
            y: None,
            z: None,
        };
        let (_, diags) = apply_action(state, PlotAction::SetMinMax(mm));
        assert!(!diags.is_empty());
    }

    #[test]
    fn set_minmax_accepts_partial_override() {
        let state = default_state();
        let mm = MinMaxOverride {
            x: None,
            y: Some(AxisBounds { min: 0.0, max: 1.0 }),
            z: None,
        };
        let (new_state, diags) = apply_action(state, PlotAction::SetMinMax(mm.clone()));
        assert_eq!(new_state.minmax, mm);
        assert!(diags.is_empty());
    }

    // ── SetContourSpec ────────────────────────────────────────────────────────

    #[test]
    fn set_contour_spec_none_clears_contours() {
        let mut state = default_state();
        state.contour_spec = ContourSpec::Automatic { count: 5 };
        let (new_state, diags) = apply_action(state, PlotAction::SetContourSpec(ContourSpec::None));
        assert_eq!(new_state.contour_spec, ContourSpec::None);
        assert!(diags.is_empty());
    }

    #[test]
    fn set_contour_spec_automatic_valid_count() {
        let state = default_state();
        let (new_state, diags) = apply_action(
            state,
            PlotAction::SetContourSpec(ContourSpec::Automatic { count: 20 }),
        );
        assert_eq!(new_state.contour_spec, ContourSpec::Automatic { count: 20 });
        assert!(diags.is_empty());
    }

    #[test]
    fn set_contour_spec_automatic_zero_count_warns_and_defaults_to_10() {
        let state = default_state();
        let (new_state, diags) = apply_action(
            state,
            PlotAction::SetContourSpec(ContourSpec::Automatic { count: 0 }),
        );
        assert_eq!(new_state.contour_spec, ContourSpec::Automatic { count: 10 });
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, DiagnosticSeverity::Warning);
        assert_eq!(diags[0].capability, cap::CONTOURS);
    }

    #[test]
    fn set_contour_spec_increment_valid() {
        let state = default_state();
        let spec = ContourSpec::Increment {
            start: 0.0,
            increment: 0.1,
        };
        let (new_state, diags) = apply_action(state, PlotAction::SetContourSpec(spec.clone()));
        assert_eq!(new_state.contour_spec, spec);
        assert!(diags.is_empty());
    }

    #[test]
    fn set_contour_spec_increment_zero_warns_and_ignores() {
        let state = default_state();
        let (new_state, diags) = apply_action(
            state,
            PlotAction::SetContourSpec(ContourSpec::Increment {
                start: 0.0,
                increment: 0.0,
            }),
        );
        assert_eq!(new_state.contour_spec, ContourSpec::None);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, DiagnosticSeverity::Warning);
    }

    #[test]
    fn set_contour_spec_manual_stores_entries() {
        let state = default_state();
        let entries = vec![
            ContourEntry {
                value: 1.5,
                color: None,
            },
            ContourEntry {
                value: 3.0,
                color: Some([1.0, 0.0, 0.0, 1.0]),
            },
        ];
        let spec = ContourSpec::Manual {
            entries: entries.clone(),
        };
        let (new_state, diags) = apply_action(state, PlotAction::SetContourSpec(spec));
        assert_eq!(new_state.contour_spec, ContourSpec::Manual { entries });
        assert!(diags.is_empty());
    }

    // ── SetWalls / SetSubsets ─────────────────────────────────────────────────

    #[test]
    fn set_walls_replaces_list() {
        let state = default_state();
        let walls = vec![GridSubset {
            grid: 1,
            gui_managed: false,
            i_range: Some(IndexRange {
                start: 1,
                end: Some(10),
            }),
            j_range: None,
            k_range: None,
        }];
        let (new_state, diags) = apply_action(state, PlotAction::SetWalls(walls.clone()));
        assert_eq!(new_state.walls, walls);
        assert!(diags.is_empty());
    }

    #[test]
    fn set_subsets_replaces_list() {
        let state = default_state();
        let subsets = vec![GridSubset {
            grid: 2,
            gui_managed: false,
            i_range: None,
            j_range: Some(IndexRange {
                start: 5,
                end: None,
            }),
            k_range: None,
        }];
        let (new_state, diags) = apply_action(state, PlotAction::SetSubsets(subsets.clone()));
        assert_eq!(new_state.subsets, subsets);
        assert!(diags.is_empty());
    }

    #[test]
    fn add_subsets_appends_list() {
        let mut state = default_state();
        state.subsets = vec![GridSubset {
            grid: 1,
            gui_managed: false,
            i_range: Some(IndexRange {
                start: 1,
                end: Some(1),
            }),
            j_range: None,
            k_range: None,
        }];

        let additions = vec![GridSubset {
            grid: 2,
            gui_managed: false,
            i_range: None,
            j_range: Some(IndexRange {
                start: 5,
                end: Some(5),
            }),
            k_range: None,
        }];

        let (new_state, diags) = apply_action(state, PlotAction::AddSubsets(additions.clone()));
        assert_eq!(new_state.subsets.len(), 2);
        assert_eq!(new_state.subsets[1], additions[0]);
        assert!(diags.is_empty());
    }

    // ── SetFsurface ───────────────────────────────────────────────────────────

    #[test]
    fn set_fsurface_stores_spec() {
        let state = default_state();
        let fs = FsurfaceSpec {
            value: 1.225,
            scalar_field: ScalarField::Density,
        };
        let (new_state, diags) = apply_action(state, PlotAction::SetFsurface(Some(fs.clone())));
        assert_eq!(new_state.fsurface, Some(fs));
        assert!(diags.is_empty());
    }

    #[test]
    fn set_fsurface_none_clears_spec() {
        let mut state = default_state();
        state.fsurface = Some(FsurfaceSpec {
            value: 1.0,
            scalar_field: ScalarField::Pressure,
        });
        let (new_state, diags) = apply_action(state, PlotAction::SetFsurface(None));
        assert!(new_state.fsurface.is_none());
        assert!(diags.is_empty());
    }

    // ── Text annotations ──────────────────────────────────────────────────────

    #[test]
    fn add_text_annotation_appends() {
        let state = default_state();
        let text = PlotText {
            content: "hello".to_owned(),
            x: 0.1,
            y: 0.9,
        };
        let (new_state, diags) = apply_action(state, PlotAction::AddTextAnnotation(text.clone()));
        assert_eq!(new_state.text_annotations.len(), 1);
        assert_eq!(new_state.text_annotations[0], text);
        assert!(diags.is_empty());
    }

    #[test]
    fn add_multiple_text_annotations_accumulate() {
        let state = default_state();
        let t1 = PlotText {
            content: "a".to_owned(),
            x: 0.0,
            y: 0.0,
        };
        let t2 = PlotText {
            content: "b".to_owned(),
            x: 0.5,
            y: 0.5,
        };
        let (s1, _) = apply_action(state, PlotAction::AddTextAnnotation(t1));
        let (s2, diags) = apply_action(s1, PlotAction::AddTextAnnotation(t2));
        assert_eq!(s2.text_annotations.len(), 2);
        assert!(diags.is_empty());
    }

    #[test]
    fn clear_text_annotations_removes_all() {
        let mut state = default_state();
        state.text_annotations.push(PlotText {
            content: "x".to_owned(),
            x: 0.0,
            y: 0.0,
        });
        let (new_state, diags) = apply_action(state, PlotAction::ClearTextAnnotations);
        assert!(new_state.text_annotations.is_empty());
        assert!(diags.is_empty());
    }

    // ── Plot mode ─────────────────────────────────────────────────────────────

    #[test]
    fn plot_mode_defaults_to_surface3d() {
        let state = default_state();
        assert_eq!(state.plot_mode, PlotMode::Surface3d);
    }

    #[test]
    fn set_plot_mode_updates_mode() {
        let state = default_state();
        let (new_state, diags) = apply_action(state, PlotAction::SetPlotMode(PlotMode::Contours));
        assert_eq!(new_state.plot_mode, PlotMode::Contours);
        assert!(diags.is_empty());
    }

    // ── CommitPlot ────────────────────────────────────────────────────────────

    #[test]
    fn commit_plot_does_not_mutate_state() {
        let state = default_state();
        let expected = state.clone();
        let (new_state, diags) = apply_action(state, PlotAction::CommitPlot);
        assert_eq!(new_state, expected);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, DiagnosticSeverity::Info);
        assert_eq!(diags[0].capability, cap::PLOT);
    }

    // ── Multiple sequential actions ───────────────────────────────────────────

    #[test]
    fn sequential_actions_compose_correctly() {
        let state = default_state();
        let (s1, _) = apply_action(state, PlotAction::SetScalarField(ScalarField::Density));
        let (s2, _) = apply_action(
            s1,
            PlotAction::SetContourSpec(ContourSpec::Automatic { count: 5 }),
        );
        let (s3, _) = apply_action(
            s2,
            PlotAction::SetMinMax(MinMaxOverride {
                x: Some(AxisBounds { min: 0.1, max: 1.2 }),
                y: None,
                z: None,
            }),
        );
        let (s4, _) = apply_action(s3, PlotAction::SetPlotMode(PlotMode::Lines));
        assert_eq!(s4.scalar_field, ScalarField::Density);
        assert_eq!(s4.contour_spec, ContourSpec::Automatic { count: 5 });
        assert_eq!(s4.minmax.x, Some(AxisBounds { min: 0.1, max: 1.2 }));
        assert_eq!(s4.plot_mode, PlotMode::Lines);
    }

    // ── apply_action is pure (original not mutated) ───────────────────────────

    #[test]
    fn apply_action_does_not_mutate_original_state() {
        let state = default_state();
        let original = state.clone();
        let _ = apply_action(
            state.clone(),
            PlotAction::SetScalarField(ScalarField::Energy),
        );
        // `state` is moved into apply_action but we verify via `original`.
        assert_eq!(original.scalar_field, ScalarField::None);
    }
}
