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
    pub const VECTORS: &str = "VECTORS";
    pub const RAKES: &str = "RAKES";
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
            // Density family
            "normalized_density" => Some(ScalarField::NormalizedDensity),
            "stagnation_density" => Some(ScalarField::StagnationDensity),
            "normalized_stagnation_density" => Some(ScalarField::NormalizedStagnationDensity),
            "log_normalized_density" => Some(ScalarField::LogNormalizedDensity),
            // Pressure family
            "normalized_pressure" => Some(ScalarField::NormalizedPressure),
            "stagnation_pressure" => Some(ScalarField::StagnationPressure),
            "normalized_stagnation_pressure" => Some(ScalarField::NormalizedStagnationPressure),
            "pressure_coefficient" => Some(ScalarField::PressureCoefficient),
            "stagnation_pressure_coefficient" => Some(ScalarField::StagnationPressureCoefficient),
            "pitot_pressure" => Some(ScalarField::PitotPressure),
            "pitot_pressure_ratio" => Some(ScalarField::PitotPressureRatio),
            "dynamic_pressure" => Some(ScalarField::DynamicPressure),
            "log_normalized_pressure" => Some(ScalarField::LogNormalizedPressure),
            // Temperature family
            "temperature" => Some(ScalarField::Temperature),
            "normalized_temperature" => Some(ScalarField::NormalizedTemperature),
            "stagnation_temperature" => Some(ScalarField::StagnationTemperature),
            "normalized_stagnation_temperature" => {
                Some(ScalarField::NormalizedStagnationTemperature)
            }
            "log_normalized_temperature" => Some(ScalarField::LogNormalizedTemperature),
            // Enthalpy family
            "enthalpy" => Some(ScalarField::Enthalpy),
            "normalized_enthalpy" => Some(ScalarField::NormalizedEnthalpy),
            "stagnation_enthalpy" => Some(ScalarField::StagnationEnthalpy),
            "normalized_stagnation_enthalpy" => Some(ScalarField::NormalizedStagnationEnthalpy),
            // Energy family
            "internal_energy" => Some(ScalarField::InternalEnergy),
            "normalized_internal_energy" => Some(ScalarField::NormalizedInternalEnergy),
            "stagnation_energy" => Some(ScalarField::StagnationEnergy),
            "normalized_stagnation_energy" => Some(ScalarField::NormalizedStagnationEnergy),
            "kinetic_energy" => Some(ScalarField::KineticEnergy),
            "normalized_kinetic_energy" => Some(ScalarField::NormalizedKineticEnergy),
            // Velocity / flow family
            "mach_number" => Some(ScalarField::MachNumber),
            "speed_of_sound" => Some(ScalarField::SpeedOfSound),
            "cross_flow_velocity" => Some(ScalarField::CrossFlowVelocity),
            "normalized_2d_stream_function" => Some(ScalarField::Normalized2dStreamFunction),
            "velocity_divergence" => Some(ScalarField::VelocityDivergence),
            // Entropy family
            "entropy" => Some(ScalarField::Entropy),
            "entropy_measure_s1" => Some(ScalarField::EntropyMeasureS1),
            // Vorticity family
            "vorticity_x" => Some(ScalarField::VorticityX),
            "vorticity_y" => Some(ScalarField::VorticityY),
            "vorticity_z" => Some(ScalarField::VorticityZ),
            "vorticity_magnitude" => Some(ScalarField::VorticityMagnitude),
            "swirl" => Some(ScalarField::Swirl),
            "velocity_cross_vorticity_magnitude" => {
                Some(ScalarField::VelocityCrossVorticityMagnitude)
            }
            "helicity_density" => Some(ScalarField::HelicityDensity),
            "relative_helicity" => Some(ScalarField::RelativeHelicity),
            "filtered_relative_helicity" => Some(ScalarField::FilteredRelativeHelicity),
            // Shock / gradient family
            "shock_function_pressure_gradient" => Some(ScalarField::ShockFunctionPressureGradient),
            "filtered_shock_function" => Some(ScalarField::FilteredShockFunction),
            "pressure_gradient_magnitude" => Some(ScalarField::PressureGradientMagnitude),
            "density_gradient_magnitude" => Some(ScalarField::DensityGradientMagnitude),
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

impl ContourSpec {
    /// Resolve this contour specification to an ordered list of absolute
    /// physical field values, using the known field range \[`min`, `max`\].
    ///
    /// This is the **one canonical resolver** that all contour-level resolution
    /// flows must go through; it must never receive or return normalized 0..1
    /// values.
    ///
    /// # Edge / degenerate cases
    /// - `None`: returns an empty list and no diagnostics.
    /// - `Automatic` with a uniform field (`|max − min| < ε`): returns
    ///   `[min]` and emits a `Warning` diagnostic.
    /// - `Increment` whose `start` is beyond `max`: returns an empty list.
    /// - `Manual`: values are passed through verbatim; range is not consulted.
    pub fn resolve(&self, min: f64, max: f64) -> (Vec<f64>, Vec<Diagnostic>) {
        let mut diags: Vec<Diagnostic> = Vec::new();

        match self {
            ContourSpec::None => (vec![], diags),

            ContourSpec::Automatic { count } => {
                let count = *count as usize;
                let span = max - min;
                if span.abs() < f64::EPSILON {
                    diags.push(Diagnostic::warning(
                        cap::CONTOURS,
                        format!(
                            "Uniform field (min == max == {min:.6e}); \
                             Automatic contours collapsed to single level at field value"
                        ),
                    ));
                    return (vec![min], diags);
                }
                // Evenly distribute `count` levels within (min, max) exclusive.
                let levels: Vec<f64> = (1..=count)
                    .map(|i| min + span * (i as f64) / (count as f64 + 1.0))
                    .collect();
                (levels, diags)
            }

            ContourSpec::Increment { start, increment } => {
                let start = *start;
                let increment = *increment;
                // increment <= 0 should have been caught at parse time; be defensive.
                if increment <= 0.0 {
                    diags.push(Diagnostic::warning(
                        cap::CONTOURS,
                        "Increment must be > 0; no contour levels resolved",
                    ));
                    return (vec![], diags);
                }
                let span = max - min;
                if span.abs() < f64::EPSILON {
                    diags.push(Diagnostic::warning(
                        cap::CONTOURS,
                        format!(
                            "Uniform field (min == max == {min:.6e}); \
                             Increment contours produce no levels in range"
                        ),
                    ));
                    return (vec![], diags);
                }
                // Advance `start` to the first level >= min.
                let first = if start < min {
                    let steps = ((min - start) / increment).ceil();
                    start + steps * increment
                } else {
                    start
                };
                let max_levels: usize = 512;
                let mut levels = Vec::new();
                let mut v = first;
                while v <= max && levels.len() < max_levels {
                    levels.push(v);
                    v += increment;
                }
                (levels, diags)
            }

            ContourSpec::Manual { entries } => {
                let levels: Vec<f64> = entries.iter().map(|e| e.value).collect();
                (levels, diags)
            }
        }
    }
}

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

/// Preferred vertical orientation for `PLOT/UP`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlotUpAxis {
    PositiveX,
    PositiveY,
    PositiveZ,
    NegativeX,
    NegativeY,
    NegativeZ,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WallRenderMode {
    Line,
    Shaded,
    HiddenLines,
    Points,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WallColor {
    White,
    Red,
    Green,
    Blue,
    Cyan,
    Magenta,
    Yellow,
    Black,
    Rgb { r: u8, g: u8, b: u8 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WallStyle {
    #[serde(default)]
    pub mode: Option<WallRenderMode>,
    #[serde(default)]
    pub color: Option<WallColor>,
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
    #[serde(default)]
    pub style: WallStyle,
}

// ──────────────────────────────────────────────────────────────────────────────
// FSURFACE
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FsurfaceSpec {
    /// Current bounded-MVP FSURFACE representation.
    ///
    /// This stores an absolute iso-level plus FUNCTION (scalar field) selection,
    /// not the full legacy FSURFACE axis-property controls such as
    /// SCALE_FACTOR, WALLS_ORIGIN, or GRID/CONTOUR behavior.
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

/// Shared VECTORS state used for deterministic parser/executor replay and
/// renderer overlays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct VectorSettings {
    /// Legacy scalar function number used to color vectors.
    pub scalar_function: Option<u16>,
    /// Whether /NOSCALAR_FUNCTION was explicitly provided.
    pub scalar_function_disabled: bool,
    /// Optional vector length multiplier from /LENGTH_SCALE.
    pub length_scale: Option<f64>,
    /// Optional attribute toggle from /(NO)ATTRIBUTES.
    pub attributes_enabled: Option<bool>,
}

/// RAKES coordinate interpretation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RakeCoordinateMode {
    Ijk,
    Xyz,
}

/// RAKES time-direction mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RakeTimeMode {
    Plus,
    Minus,
    PlusMinus,
}

/// RAKES read/write mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "path", rename_all = "snake_case")]
pub enum RakeIoMode {
    Read(String),
    Write(String),
}

/// Shared RAKES state. Geometry/particle rendering is deferred; parser/executor
/// preserve deterministic command intent in PlotState.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RakeSettings {
    pub coordinate_mode: Option<RakeCoordinateMode>,
    pub add: bool,
    pub attributes_enabled: Option<bool>,
    pub io_mode: Option<RakeIoMode>,
    pub time_mode: Option<RakeTimeMode>,
    pub max_points: Option<u32>,
    pub scalar_function: Option<u16>,
    pub scalar_function_disabled: bool,
}

// ──────────────────────────────────────────────────────────────────────────────
// Contour attribute (CONTOURS attribute qualifier)
// ──────────────────────────────────────────────────────────────────────────────

/// How contour levels are visually rendered.  Set through the `CONTOURS`
/// command's attribute qualifiers.  The default is `Line`.
///
/// These correspond to the legacy "contour attribute type" concept used by both
/// contour plots and function-surface plots (the CONTOURS command controls
/// attribute for both families).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContourAttribute {
    /// Line contours (default).
    Line,
    /// Filled polygon surface.
    Surface,
    /// Grid mesh lines.
    Grid,
    /// Filled contours using the field colormap.
    ColorContours,
    /// Dot representation.
    Dots,
}

impl Default for ContourAttribute {
    fn default() -> Self {
        ContourAttribute::Line
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Plot family (PLOT qualifier)
// ──────────────────────────────────────────────────────────────────────────────

/// Which visualization family the current `PLOT` command selects.
///
/// Replaces the legacy string-keyed "plot mode" with explicit semantics:
/// - `Contour` (default) — scalar levels drawn on geometry (`PLOT/CONTOUR`).
/// - `FunctionSurface` — function value plotted as a spatial dimension
///   (`PLOT/SURFACE`, `PLOT/CARPET`, or the 2D-degenerate `PLOT/LINE`).
///
/// Plot-family selection is owned by this enum and must not be re-encoded
/// through retired UI shortcuts such as an "Enable Contours" checkbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlotFamily {
    /// Contour plot — scalar iso-levels on a mesh surface (default).
    Contour,
    /// Function-surface / carpet plot — scalar value treated as a spatial axis.
    FunctionSurface,
}

impl Default for PlotFamily {
    fn default() -> Self {
        PlotFamily::Contour
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Non-scalar function types (Phase E — 0-99, 200-299, 300-399, 400+)
// ──────────────────────────────────────────────────────────────────────────────

/// Grid-diagnostic / geometry visualization modes (FUNCTION 0–99).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GridFunction {
    /// FUNCTION 0 — walls alone (geometry only).
    Walls,
    /// FUNCTION 1 — all grids.
    Grids,
    /// FUNCTION 2 — IBLANK hole outlines.
    IBlankHoles,
    /// FUNCTION 3 — hole-boundary orphan points.
    OrphanPoints,
    /// FUNCTION 10 — 2D crossing grid-line check.
    CrossingGridLineCheck,
    /// FUNCTION 11 — tetrahedron decomposition cell volume check.
    TetDecompositionVolumeCheck,
    /// FUNCTION 12 — tetrahedron decomposition grid crossing check.
    TetDecompositionCrossingCheck,
}

/// Vector field selections (FUNCTION 200–299).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VectorField {
    /// FUNCTION 200 — velocity vector (u, v, w).
    Velocity,
    /// FUNCTION 201 — vorticity vector (ωₓ, ω_y, ω_z).
    Vorticity,
    /// FUNCTION 202 — momentum vector (Q2, Q3, Q4).
    Momentum,
    /// FUNCTION 203 — perturbation velocity V′ = V − V∞.
    PerturbationVelocity,
    /// FUNCTION 204 — velocity × vorticity vector.
    VelocityCrossVorticity,
    /// FUNCTION 210 — pressure gradient vector ∇p.
    PressureGradient,
    /// FUNCTION 211 — density gradient vector ∇ρ.
    DensityGradient,
}

/// Particle / stream-trace function selections (FUNCTION 300–399).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticleFunction {
    /// FUNCTION 300 — particle traces (trilinear + RK2 advection).
    ParticleTraces,
    /// FUNCTION 301 — vortex lines (advected by vorticity).
    VortexLines,
}

/// Special overlay function selections (FUNCTION 400+).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecialFunction {
    /// FUNCTION 400 — shock locations based on pressure gradient.
    ShockByPressureGradient,
    /// FUNCTION 401 — filtered shock locations based on pressure gradient.
    FilteredShockByPressureGradient,
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

    // PLOT/UP
    pub plot_up: Option<PlotUpAxis>,

    // MINMAX
    pub minmax: MinMaxOverride,

    // CONTOURS: level specification
    pub contour_spec: ContourSpec,
    // CONTOURS: visual attribute (line/surface/grid/color/dots)
    pub contour_attribute: ContourAttribute,

    // WALLS
    pub walls: Vec<GridSubset>,

    // SUBSETS
    pub subsets: Vec<GridSubset>,

    // FSURFACE
    pub fsurface: Option<FsurfaceSpec>,

    // TEXT
    pub text_annotations: Vec<PlotText>,

    // VECTORS
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vectors: Option<VectorSettings>,

    // RAKES
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rakes: Option<RakeSettings>,

    // PLOT
    pub plot_family: PlotFamily,

    // FUNCTION (non-scalar ranges — Phase E)
    // These are set by FUNCTION N for N outside 100-199.
    // Rendering is deferred; state is tracked for determinism.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid_function: Option<GridFunction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector_field: Option<VectorField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub particle_function: Option<ParticleFunction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub special_function: Option<SpecialFunction>,
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

    // FUNCTION 100-199: choose which scalar field to visualise.
    SetScalarField(ScalarField),

    // FUNCTION 0-99: select a grid-diagnostic / geometry mode.
    SetGridFunction(GridFunction),

    // FUNCTION 200-299: select a vector field (rendering deferred).
    SetVectorField(VectorField),

    // FUNCTION 300-399: select a particle/stream-trace function (rendering deferred).
    SetParticleTrace(ParticleFunction),

    // FUNCTION 400+: select a special overlay function (rendering deferred).
    SetSpecialFunction(SpecialFunction),

    // VIEW: select a named axis-aligned camera preset.
    SetAxisView(AxisView),

    // VPOINT: set an explicit camera look-from point.
    SetViewpoint(ViewPoint),

    // PLOT/UP: set the preferred vertical plot axis.
    SetPlotUpAxis(PlotUpAxis),

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

    // FSURFACE: set or clear the bounded-MVP iso-level + FUNCTION spec.
    SetFsurface(Option<FsurfaceSpec>),

    // TEXT: append a text annotation.
    AddTextAnnotation(PlotText),

    // TEXT: clear all text annotations.
    ClearTextAnnotations,

    // VECTORS: update vector-display settings (rendering deferred).
    SetVectors(VectorSettings),

    // RAKES: update rake/particle-seed settings (rendering deferred).
    SetRakes(RakeSettings),

    // SHOW: emit a status snapshot in executor output.
    ShowStatus,

    // PLOT: choose between contour and function-surface families.
    SetPlotFamily(PlotFamily),

    // CONTOURS: set the visual rendering attribute (line/surface/grid/etc.).
    SetContourAttribute(ContourAttribute),

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

        PlotAction::SetPlotUpAxis(axis) => {
            state.plot_up = Some(axis);
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
            println!("SetWalls action: {:?}", state.walls);
        }

        PlotAction::AddWalls(mut walls) => {
            state.walls.append(&mut walls);
            println!("AddWalls action: {:?}", state.walls);
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

        PlotAction::SetVectors(settings) => {
            state.vectors = Some(settings);
            diags.push(Diagnostic::info(
                cap::VECTORS,
                "VECTORS settings captured (rendering deferred)",
            ));
        }

        PlotAction::SetRakes(settings) => {
            state.rakes = Some(settings);
            diags.push(Diagnostic::info(
                cap::RAKES,
                "RAKES settings captured (rendering deferred)",
            ));
        }

        PlotAction::ShowStatus => {
            // SHOW does not mutate PlotState; executor owns output formatting.
            diags.push(Diagnostic::info(cap::SHOW, "Show status requested"));
        }

        PlotAction::SetPlotFamily(family) => {
            state.plot_family = family;
        }

        PlotAction::SetContourAttribute(attr) => {
            state.contour_attribute = attr;
        }

        PlotAction::CommitPlot => {
            // CommitPlot itself does not mutate state; the executor layer is
            // responsible for deriving a RenderIntent from the current state
            // when it handles this action.  We emit an info diagnostic so
            // callers can observe the boundary in a diagnostic stream.
            diags.push(Diagnostic::info(cap::PLOT, "PLOT committed"));

            // Warn on unsupported combinations so callers can surface them.
            if state.plot_family == PlotFamily::FunctionSurface
                && state.contour_spec != ContourSpec::None
            {
                diags.push(Diagnostic::warning(
                    cap::CONTOURS,
                    "CONTOURS spec is ignored when PLOT/SURFACE (CARPET/LINE) is active; switch to PLOT/CONTOUR to use contour levels.",
                ));
            }
            if state.plot_family == PlotFamily::Contour
                && matches!(
                    state.contour_attribute,
                    ContourAttribute::Grid | ContourAttribute::Dots
                )
            {
                diags.push(Diagnostic::warning(
                    cap::CONTOURS,
                    "GRID and DOTS CONTOURS attributes are not fully implemented; rendering as LINE contours.",
                ));
            }
        }

        PlotAction::SetGridFunction(gf) => {
            state.grid_function = Some(gf);
        }

        PlotAction::SetVectorField(vf) => {
            state.vector_field = Some(vf);
        }

        PlotAction::SetParticleTrace(pf) => {
            state.particle_function = Some(pf);
        }

        PlotAction::SetSpecialFunction(sf) => {
            state.special_function = Some(sf);
        }
    }

    (state, diags)
}

// ──────────────────────────────────────────────────────────────────────────────
// Coordinate conversion helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Convert spherical coordinates (DISSPLA convention) to Cartesian.
///
/// DISSPLA convention:
/// - phi: azimuth angle in horizontal plane (degrees), measured from +X toward +Y.
/// - theta: elevation angle above horizontal plane (degrees).
/// - radius: distance from origin.
///
/// Returns (x, y, z) Cartesian coordinates.
pub fn spherical_to_cartesian(phi_deg: f64, theta_deg: f64, radius: f64) -> (f64, f64, f64) {
    let phi_rad = phi_deg.to_radians();
    let theta_rad = theta_deg.to_radians();

    let xy_distance = theta_rad.cos(); // Horizontal distance from Z axis
    let z = theta_rad.sin() * radius;

    let x = xy_distance * phi_rad.cos() * radius;
    let y = xy_distance * phi_rad.sin() * radius;

    (x, y, z)
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

    #[test]
    fn set_plot_up_axis_stores_orientation() {
        let state = default_state();
        let (new_state, diags) =
            apply_action(state, PlotAction::SetPlotUpAxis(PlotUpAxis::NegativeY));
        assert_eq!(new_state.plot_up, Some(PlotUpAxis::NegativeY));
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
        let (new_state, diags) = apply_action(state, PlotAction::SetContourSpec(spec.clone()));
        assert_eq!(new_state.contour_spec, spec);
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
            style: WallStyle::default(),
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
            style: WallStyle::default(),
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
            style: WallStyle::default(),
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
            style: WallStyle::default(),
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

    #[test]
    fn set_vectors_stores_settings_and_emits_info() {
        let state = default_state();
        let vectors = VectorSettings {
            scalar_function: Some(114),
            scalar_function_disabled: false,
            length_scale: Some(0.75),
            attributes_enabled: Some(true),
        };

        let (new_state, diags) = apply_action(state, PlotAction::SetVectors(vectors.clone()));
        assert_eq!(new_state.vectors, Some(vectors));
        assert!(diags.iter().any(|d| d.capability == cap::VECTORS));
    }

    #[test]
    fn set_rakes_stores_settings_and_emits_info() {
        let state = default_state();
        let rakes = RakeSettings {
            coordinate_mode: Some(RakeCoordinateMode::Xyz),
            add: true,
            attributes_enabled: Some(false),
            io_mode: Some(RakeIoMode::Write("out.rake".to_string())),
            time_mode: Some(RakeTimeMode::PlusMinus),
            max_points: Some(400),
            scalar_function: Some(190),
            scalar_function_disabled: false,
        };

        let (new_state, diags) = apply_action(state, PlotAction::SetRakes(rakes.clone()));
        assert_eq!(new_state.rakes, Some(rakes));
        assert!(diags.iter().any(|d| d.capability == cap::RAKES));
    }

    // ── Plot family ───────────────────────────────────────────────────────────

    #[test]
    fn plot_family_defaults_to_contour() {
        let state = default_state();
        assert_eq!(state.plot_family, PlotFamily::Contour);
    }

    #[test]
    fn plot_up_defaults_to_none() {
        let state = default_state();
        assert_eq!(state.plot_up, None);
    }

    #[test]
    fn set_plot_family_updates_family() {
        let state = default_state();
        let (new_state, diags) = apply_action(
            state,
            PlotAction::SetPlotFamily(PlotFamily::FunctionSurface),
        );
        assert_eq!(new_state.plot_family, PlotFamily::FunctionSurface);
        assert!(diags.is_empty());
    }

    // ── Contour attribute ─────────────────────────────────────────────────────

    #[test]
    fn contour_attribute_defaults_to_line() {
        let state = default_state();
        assert_eq!(state.contour_attribute, ContourAttribute::Line);
    }

    #[test]
    fn set_contour_attribute_updates_attribute() {
        let state = default_state();
        let (new_state, diags) = apply_action(
            state,
            PlotAction::SetContourAttribute(ContourAttribute::Surface),
        );
        assert_eq!(new_state.contour_attribute, ContourAttribute::Surface);
        assert!(diags.is_empty());
    }

    #[test]
    fn set_contour_attribute_color_contours() {
        let state = default_state();
        let (new_state, diags) = apply_action(
            state,
            PlotAction::SetContourAttribute(ContourAttribute::ColorContours),
        );
        assert_eq!(new_state.contour_attribute, ContourAttribute::ColorContours);
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
        let (s4, _) = apply_action(s3, PlotAction::SetPlotFamily(PlotFamily::FunctionSurface));
        assert_eq!(s4.scalar_field, ScalarField::Density);
        assert_eq!(s4.contour_spec, ContourSpec::Automatic { count: 5 });
        assert_eq!(s4.minmax.x, Some(AxisBounds { min: 0.1, max: 1.2 }));
        assert_eq!(s4.plot_family, PlotFamily::FunctionSurface);
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

    // ── ContourSpec::resolve ──────────────────────────────────────────────────

    #[test]
    fn resolve_none_returns_empty() {
        let (levels, diags) = ContourSpec::None.resolve(0.0, 1.0);
        assert!(levels.is_empty());
        assert!(diags.is_empty());
    }

    #[test]
    fn resolve_automatic_distributes_evenly_inside_range() {
        // count=4 → 4 levels at t = 1/5, 2/5, 3/5, 4/5 of [0, 10]
        let (levels, diags) = ContourSpec::Automatic { count: 4 }.resolve(0.0, 10.0);
        assert_eq!(levels.len(), 4);
        assert!(diags.is_empty());
        let expected = [2.0, 4.0, 6.0, 8.0];
        for (got, want) in levels.iter().zip(expected.iter()) {
            assert!((got - want).abs() < 1e-10, "expected {want} got {got}");
        }
        // All levels are strictly inside (min, max)
        for &v in &levels {
            assert!(v > 0.0 && v < 10.0, "level {v} not inside (0, 10)");
        }
    }

    #[test]
    fn resolve_automatic_count_one() {
        let (levels, diags) = ContourSpec::Automatic { count: 1 }.resolve(0.0, 100.0);
        assert_eq!(levels.len(), 1);
        assert!(diags.is_empty());
        assert!((levels[0] - 50.0).abs() < 1e-10);
    }

    #[test]
    fn resolve_automatic_uniform_field_emits_warning_and_single_level() {
        let (levels, diags) = ContourSpec::Automatic { count: 5 }.resolve(3.0, 3.0);
        assert_eq!(levels.len(), 1);
        assert!(
            (levels[0] - 3.0).abs() < 1e-10,
            "level should be the uniform value 3.0"
        );
        assert_eq!(diags.len(), 1);
        assert!(matches!(diags[0].severity, DiagnosticSeverity::Warning));
    }

    #[test]
    fn resolve_increment_normal_range() {
        // start=1.0, inc=0.5, range [0, 3] → levels: 1.0, 1.5, 2.0, 2.5, 3.0
        let spec = ContourSpec::Increment {
            start: 1.0,
            increment: 0.5,
        };
        let (levels, diags) = spec.resolve(0.0, 3.0);
        assert!(diags.is_empty());
        let expected = [1.0, 1.5, 2.0, 2.5, 3.0];
        assert_eq!(levels.len(), expected.len());
        for (got, want) in levels.iter().zip(expected.iter()) {
            assert!((got - want).abs() < 1e-10, "expected {want} got {got}");
        }
    }

    #[test]
    fn resolve_increment_start_below_min_advances_to_first_in_range() {
        // start=-5.0, inc=2.0, range [0, 6] → first >= 0 is 1.0? No:
        // start=-5, steps = ceil((0 - (-5)) / 2.0) = ceil(2.5) = 3, first = -5 + 3*2 = 1
        // levels: 1.0, 3.0, 5.0
        let spec = ContourSpec::Increment {
            start: -5.0,
            increment: 2.0,
        };
        let (levels, diags) = spec.resolve(0.0, 6.0);
        assert!(diags.is_empty());
        assert_eq!(levels, vec![1.0, 3.0, 5.0]);
    }

    #[test]
    fn resolve_increment_start_beyond_max_returns_empty() {
        let spec = ContourSpec::Increment {
            start: 10.0,
            increment: 1.0,
        };
        let (levels, diags) = spec.resolve(0.0, 5.0);
        assert!(levels.is_empty());
        assert!(diags.is_empty());
    }

    #[test]
    fn resolve_increment_negative_increment_returns_empty_with_warning() {
        let spec = ContourSpec::Increment {
            start: 0.0,
            increment: -1.0,
        };
        let (levels, diags) = spec.resolve(0.0, 5.0);
        assert!(levels.is_empty());
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn resolve_increment_uniform_field_returns_empty_with_warning() {
        let spec = ContourSpec::Increment {
            start: 0.0,
            increment: 0.5,
        };
        let (levels, diags) = spec.resolve(2.0, 2.0);
        assert!(levels.is_empty());
        assert_eq!(diags.len(), 1);
        assert!(matches!(diags[0].severity, DiagnosticSeverity::Warning));
    }

    #[test]
    fn resolve_manual_passthrough_ignores_range() {
        let spec = ContourSpec::Manual {
            entries: vec![
                ContourEntry {
                    value: -99.0,
                    color: None,
                },
                ContourEntry {
                    value: 0.0,
                    color: None,
                },
                ContourEntry {
                    value: 42.5,
                    color: None,
                },
            ],
        };
        // Range should have zero effect on Manual levels
        let (levels, diags) = spec.resolve(0.0, 1.0);
        assert!(diags.is_empty());
        assert_eq!(levels, vec![-99.0, 0.0, 42.5]);
    }

    #[test]
    fn resolve_manual_empty_entries_returns_empty() {
        let spec = ContourSpec::Manual { entries: vec![] };
        let (levels, diags) = spec.resolve(0.0, 1.0);
        assert!(levels.is_empty());
        assert!(diags.is_empty());
    }

    // ── Normalized-contour regression (AC#1) ──────────────────────────────────
    // Manual entries must be treated as absolute physical values regardless of
    // field range.  A value that looks like a fraction (e.g. 0.5) must NOT be
    // re-mapped as a fraction of [min, max].

    #[test]
    fn resolve_manual_normalized_appearing_value_is_absolute_not_fraction() {
        // 0.5 looks like "50%" but must be returned as the absolute value 0.5,
        // not re-mapped to 0.5 * (200 - 100) + 100 = 150.
        let spec = ContourSpec::Manual {
            entries: vec![ContourEntry {
                value: 0.5,
                color: None,
            }],
        };
        let (levels, diags) = spec.resolve(100.0, 200.0);
        assert!(diags.is_empty());
        assert_eq!(levels.len(), 1);
        assert!(
            (levels[0] - 0.5).abs() < 1e-10,
            "expected absolute 0.5 but got {}",
            levels[0]
        );
    }

    #[test]
    fn resolve_manual_out_of_field_range_passes_through_unchanged() {
        // A level outside [min, max] must still be returned as-is — no clamping.
        let spec = ContourSpec::Manual {
            entries: vec![
                ContourEntry {
                    value: -50.0,
                    color: None,
                },
                ContourEntry {
                    value: 999.0,
                    color: None,
                },
            ],
        };
        let (levels, diags) = spec.resolve(0.0, 100.0);
        assert!(diags.is_empty());
        assert_eq!(levels, vec![-50.0, 999.0]);
    }

    // ── CommitPlot unsupported-combination diagnostics (AC#3) ─────────────────

    #[test]
    fn commit_plot_function_surface_with_active_contour_spec_emits_warning() {
        let mut state = default_state();
        state.plot_family = PlotFamily::FunctionSurface;
        state.contour_spec = ContourSpec::Automatic { count: 5 };

        let (new_state, diags) = apply_action(state, PlotAction::CommitPlot);

        // State is never mutated by CommitPlot.
        assert_eq!(new_state.plot_family, PlotFamily::FunctionSurface);

        // Must have the info diagnostic + the unsupported-combination warning.
        let caps: Vec<&str> = diags.iter().map(|d| d.capability.as_str()).collect();
        assert!(
            caps.contains(&cap::PLOT),
            "expected PLOT info diagnostic, got {:?}",
            caps
        );
        let warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning && d.capability == cap::CONTOURS)
            .collect();
        assert_eq!(
            warnings.len(),
            1,
            "expected exactly one CONTOURS warning, got {:?}",
            diags
        );
    }

    #[test]
    fn commit_plot_function_surface_with_none_contour_spec_no_warning() {
        let mut state = default_state();
        state.plot_family = PlotFamily::FunctionSurface;
        state.contour_spec = ContourSpec::None;

        let (_, diags) = apply_action(state, PlotAction::CommitPlot);

        let warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning)
            .collect();
        assert!(
            warnings.is_empty(),
            "expected no warnings for FunctionSurface + None spec, got {:?}",
            warnings
        );
    }

    #[test]
    fn commit_plot_contour_family_with_grid_attribute_warns() {
        let mut state = default_state();
        state.plot_family = PlotFamily::Contour;
        state.contour_attribute = ContourAttribute::Grid;

        let (_, diags) = apply_action(state, PlotAction::CommitPlot);

        let warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning && d.capability == cap::CONTOURS)
            .collect();
        assert_eq!(
            warnings.len(),
            1,
            "expected one CONTOURS warning for Grid attribute, got {:?}",
            diags
        );
    }

    #[test]
    fn commit_plot_contour_family_with_dots_attribute_warns() {
        let mut state = default_state();
        state.plot_family = PlotFamily::Contour;
        state.contour_attribute = ContourAttribute::Dots;

        let (_, diags) = apply_action(state, PlotAction::CommitPlot);

        let warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning && d.capability == cap::CONTOURS)
            .collect();
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn commit_plot_contour_family_with_line_attribute_no_warning() {
        let state = default_state(); // PlotFamily::Contour + ContourAttribute::Line
        let (_, diags) = apply_action(state, PlotAction::CommitPlot);
        let warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning)
            .collect();
        assert!(warnings.is_empty());
    }

    // ── Spherical-to-Cartesian Conversion ─────────────────────────────────────

    #[test]
    fn spherical_45_45_10_converts_correctly() {
        // φ=45°, θ=45°, r=10 should give approximately (5.0, 5.0, 7.07)
        let (x, y, z) = spherical_to_cartesian(45.0, 45.0, 10.0);
        assert!((x - 5.0).abs() < 0.01, "x mismatch: {}", x);
        assert!((y - 5.0).abs() < 0.01, "y mismatch: {}", y);
        assert!((z - 7.07).abs() < 0.01, "z mismatch: {}", z);
    }

    #[test]
    fn spherical_0_0_10_looks_towards_positive_x() {
        // φ=0°, θ=0°, r=10: azimuth 0, no elevation → (10, 0, 0)
        let (x, y, z) = spherical_to_cartesian(0.0, 0.0, 10.0);
        assert!((x - 10.0).abs() < 1e-10, "x={} should be 10.0", x);
        assert!(y.abs() < 1e-10, "y={} should be 0.0", y);
        assert!(z.abs() < 1e-10, "z={} should be 0.0", z);
    }

    #[test]
    fn spherical_90_0_10_looks_towards_positive_y() {
        // φ=90°, θ=0°, r=10: azimuth 90°, no elevation → (0, 10, 0)
        let (x, y, z) = spherical_to_cartesian(90.0, 0.0, 10.0);
        assert!(x.abs() < 1e-10, "x={} should be 0.0", x);
        assert!((y - 10.0).abs() < 1e-10, "y={} should be 10.0", y);
        assert!(z.abs() < 1e-10, "z={} should be 0.0", z);
    }

    #[test]
    fn spherical_0_90_10_looks_straight_up() {
        // φ=0° (irrelevant), θ=90°, r=10: full elevation → (0, 0, 10)
        let (x, y, z) = spherical_to_cartesian(0.0, 90.0, 10.0);
        assert!(x.abs() < 1e-10, "x={} should be 0.0", x);
        assert!(y.abs() < 1e-10, "y={} should be 0.0", y);
        assert!((z - 10.0).abs() < 1e-10, "z={} should be 10.0", z);
    }

    #[test]
    fn spherical_0_neg90_10_looks_straight_down() {
        // φ=0° (irrelevant), θ=-90°, r=10: full negative elevation → (0, 0, -10)
        let (x, y, z) = spherical_to_cartesian(0.0, -90.0, 10.0);
        assert!(x.abs() < 1e-10, "x={} should be 0.0", x);
        assert!(y.abs() < 1e-10, "y={} should be 0.0", y);
        assert!((z + 10.0).abs() < 1e-10, "z={} should be -10.0", z);
    }

    #[test]
    fn spherical_180_0_10_looks_towards_negative_x() {
        // φ=180°, θ=0°, r=10: azimuth 180°, no elevation → (-10, 0, 0)
        let (x, y, z) = spherical_to_cartesian(180.0, 0.0, 10.0);
        assert!((x + 10.0).abs() < 1e-10, "x={} should be -10.0", x);
        assert!(y.abs() < 1e-10, "y={} should be 0.0", y);
        assert!(z.abs() < 1e-10, "z={} should be 0.0", z);
    }

    #[test]
    fn spherical_270_0_10_looks_towards_negative_y() {
        // φ=270°, θ=0°, r=10: azimuth 270° (equiv -90°), no elevation → (0, -10, 0)
        let (x, y, z) = spherical_to_cartesian(270.0, 0.0, 10.0);
        assert!(x.abs() < 1e-10, "x={} should be 0.0", x);
        assert!((y + 10.0).abs() < 1e-10, "y={} should be -10.0", y);
        assert!(z.abs() < 1e-10, "z={} should be 0.0", z);
    }

    #[test]
    fn spherical_30_30_10_north_northeast_elevated() {
        // φ=30°, θ=30°, r=10: NE azimuth at 30° elevation
        let (x, _y, z) = spherical_to_cartesian(30.0, 30.0, 10.0);
        // x = cos(θ) * cos(φ) * r = cos(30°) * cos(30°) * 10 ≈ 0.866 * 0.866 * 10 ≈ 7.5
        assert!((x - 7.5).abs() < 0.1, "x should be ~7.5, got {}", x);
        // z = sin(θ) * r = sin(30°) * 10 = 0.5 * 10 = 5.0
        assert!((z - 5.0).abs() < 1e-10, "z should be 5.0, got {}", z);
    }

    #[test]
    fn spherical_conversion_symmetry_phi_360() {
        // φ=0 and φ=360 should give same result
        let (x1, y1, z1) = spherical_to_cartesian(0.0, 45.0, 10.0);
        let (x2, y2, z2) = spherical_to_cartesian(360.0, 45.0, 10.0);
        assert!((x1 - x2).abs() < 1e-10);
        assert!((y1 - y2).abs() < 1e-10);
        assert!((z1 - z2).abs() < 1e-10);
    }

    #[test]
    fn spherical_zero_radius_returns_origin() {
        // Radius 0 should give origin regardless of angles
        let (x, y, z) = spherical_to_cartesian(45.0, 45.0, 0.0);
        assert!(x.abs() < 1e-10);
        assert!(y.abs() < 1e-10);
        assert!(z.abs() < 1e-10);
    }

    #[test]
    fn spherical_negative_radius_reflection() {
        // Negative radius should flip direction
        let (x1, y1, z1) = spherical_to_cartesian(45.0, 45.0, 10.0);
        let (x2, y2, z2) = spherical_to_cartesian(45.0, 45.0, -10.0);
        assert!((x1 + x2).abs() < 1e-10, "x components should cancel");
        assert!((y1 + y2).abs() < 1e-10, "y components should cancel");
        assert!((z1 + z2).abs() < 1e-10, "z components should cancel");
    }
}
