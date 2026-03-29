use crate::com_parser::ParsedScript;
use crate::plot_state::{apply_action, Diagnostic, PlotAction, PlotState};
use serde::{Deserialize, Serialize};

/// Deterministic render payload emitted at each PLOT commit boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderIntent {
    pub state: PlotState,
    /// Loaded solution data captured at this PLOT boundary.
    ///
    /// Populated by the headless CLI after executing the script; never
    /// present in in-app IPC payloads (the app has its own grid cache).
    /// `serde(skip)` keeps serialized `RenderIntent` values compact.
    #[serde(skip)]
    pub snapshot: Option<SolutionSnapshot>,
}

impl RenderIntent {
    fn from_state(state: &PlotState) -> Self {
        Self {
            state: state.clone(),
            snapshot: None,
        }
    }
}

/// A resolved snapshot of grid geometry and a computed scalar field,
/// captured at a single PLOT commit boundary.
///
/// All coordinate and scalar arrays are flat with ordering
/// `idx = i + j*ni + k*ni*nj` where `i ∈ [0, ni)`, `j ∈ [0, nj)`,
/// `k ∈ [0, nk)`.
#[derive(Debug, Clone, PartialEq)]
pub struct SolutionSnapshot {
    /// Grid point counts along each axis.
    pub ni: u32,
    pub nj: u32,
    pub nk: u32,
    /// Flat grid X coordinates, length = ni × nj × nk.
    pub x: Vec<f32>,
    /// Flat grid Y coordinates, same length as `x`.
    pub y: Vec<f32>,
    /// Flat grid Z coordinates, same length as `x`.
    pub z: Vec<f32>,
    /// Computed scalar field values, same length as `x`.
    pub scalar: Vec<f32>,
    /// Minimum finite scalar value in this snapshot.
    pub field_min: f32,
    /// Maximum finite scalar value in this snapshot.
    pub field_max: f32,
}

/// Result of executing a parsed script against PlotState.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptExecutionResult {
    pub final_state: PlotState,
    pub intents: Vec<RenderIntent>,
    pub show_output: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Execute parsed actions in order, mutating state and emitting render intents.
///
/// Render intents are emitted only when `CommitPlot` is encountered.
pub fn execute_actions(initial_state: PlotState, actions: &[PlotAction]) -> ScriptExecutionResult {
    let mut state = initial_state;
    let mut intents: Vec<RenderIntent> = Vec::new();
    let mut show_output: Vec<String> = Vec::new();
    let mut diagnostics: Vec<Diagnostic> = Vec::new();

    for action in actions {
        if matches!(action, PlotAction::ShowStatus) {
            show_output.push(format_show_output(&state));
        }

        let (new_state, mut action_diags) = apply_action(state, action.clone());
        state = new_state;

        if matches!(action, PlotAction::CommitPlot) {
            intents.push(RenderIntent::from_state(&state));
        }

        diagnostics.append(&mut action_diags);
    }

    ScriptExecutionResult {
        final_state: state,
        intents,
        show_output,
        diagnostics,
    }
}

/// Execute a fully parsed script, preserving parser diagnostics and appending
/// execution diagnostics.
pub fn execute_parsed_script(
    initial_state: PlotState,
    parsed: &ParsedScript,
) -> ScriptExecutionResult {
    let mut result = execute_actions(initial_state, &parsed.actions);
    result.diagnostics.splice(0..0, parsed.diagnostics.clone());
    result
}

fn format_show_output(state: &PlotState) -> String {
    format!(
        "SHOW: field={:?}, family={:?}, axis_view={:?}, plot_up={:?}, text_annotations={}, walls={}, subsets={}",
        state.scalar_field,
        state.plot_family,
        state.axis_view,
        state.plot_up,
        state.text_annotations.len(),
        state.walls.len(),
        state.subsets.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::com_parser::parse_com_file;
    use crate::plot_state::{
        AxisView, DiagnosticSeverity, PlotFamily, PlotState, PlotText, ScalarField,
    };
    use serde::Deserialize;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;

    #[derive(Debug, Deserialize)]
    struct ExpectedParityFixture {
        final_state: PlotState,
        intents: Vec<RenderIntent>,
        show_output: Vec<String>,
    }

    struct ParityFixtureCase {
        name: &'static str,
        capabilities: &'static [&'static str],
    }

    const REQUIRED_CAPABILITIES: &[&str] = &[
        "FUNCTION", "VIEW", "VPOINT", "MINMAX", "CONTOURS", "PLOT", "WALLS", "SUBSETS", "FSURFACE",
        "TEXT", "SHOW",
    ];

    const PARITY_FIXTURES: &[ParityFixtureCase] = &[
        ParityFixtureCase {
            name: "full_parity_session",
            capabilities: &[
                "READ", "FUNCTION", "VIEW", "VPOINT", "MINMAX", "CONTOURS", "PLOT", "WALLS",
                "SUBSETS", "FSURFACE", "TEXT", "SHOW",
            ],
        },
        ParityFixtureCase {
            name: "contour_mode_multiplot",
            capabilities: &["CONTOURS", "PLOT"],
        },
        ParityFixtureCase {
            name: "plot_up_multiplot",
            capabilities: &["VIEW", "VPOINT", "PLOT"],
        },
        ParityFixtureCase {
            name: "function_surface_line_family",
            capabilities: &["FUNCTION", "VIEW", "MINMAX", "FSURFACE", "PLOT"],
        },
    ];

    fn parity_fixture_dir() -> PathBuf {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let candidates = [
            manifest_dir.join("tests/fixtures/parity"),
            manifest_dir.join("../src-tauri/tests/fixtures/parity"),
            manifest_dir.join("../tests/fixtures/parity"),
        ];

        candidates
            .iter()
            .find(|path| path.exists())
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "failed to locate parity fixture directory; checked: {}",
                    candidates
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
    }

    fn load_expected_fixture(case_name: &str) -> ExpectedParityFixture {
        let path = parity_fixture_dir().join(format!("{case_name}.expected.json"));
        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        serde_json::from_str(&raw)
            .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
    }

    #[test]
    fn backend_parity_fixtures_match_expected_outputs() {
        let mut covered_capabilities = BTreeSet::new();

        for case in PARITY_FIXTURES {
            covered_capabilities.extend(case.capabilities.iter().copied());

            let script_path = parity_fixture_dir().join(format!("{}.com", case.name));
            let parsed = parse_com_file(&script_path).unwrap_or_else(|error| {
                panic!("failed to parse {}: {error}", script_path.display())
            });
            let expected = load_expected_fixture(case.name);

            let first = execute_parsed_script(PlotState::default(), &parsed);
            let second = execute_parsed_script(PlotState::default(), &parsed);

            let errors: Vec<_> = first
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
                .collect();
            assert!(
                errors.is_empty(),
                "fixture '{}' should not emit errors: {:?}",
                case.name,
                errors
            );

            let warnings: Vec<_> = first
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
                .collect();
            assert!(
                warnings.is_empty(),
                "fixture '{}' should not emit warnings: {:?}",
                case.name,
                warnings
            );

            assert_eq!(
                first.final_state, expected.final_state,
                "fixture '{}' final PlotState drifted",
                case.name
            );
            assert_eq!(
                first.intents, expected.intents,
                "fixture '{}' RenderIntent output drifted",
                case.name
            );
            assert_eq!(
                first.show_output, expected.show_output,
                "fixture '{}' SHOW output drifted",
                case.name
            );

            assert_eq!(
                first.final_state, second.final_state,
                "fixture '{}' final PlotState is not deterministic across repeated executions",
                case.name
            );
            assert_eq!(
                first.intents, second.intents,
                "fixture '{}' RenderIntent output is not deterministic across repeated executions",
                case.name
            );
            assert_eq!(
                first.show_output, second.show_output,
                "fixture '{}' SHOW output is not deterministic across repeated executions",
                case.name
            );
        }

        let required_capabilities = REQUIRED_CAPABILITIES
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        assert!(
            required_capabilities.is_subset(&covered_capabilities),
            "parity fixtures must cover every required TKT-012B capability family"
        );
    }

    #[test]
    fn plot_is_only_render_intent_boundary() {
        let initial = PlotState::default();
        let actions = vec![
            PlotAction::SetScalarField(ScalarField::Density),
            PlotAction::SetPlotFamily(PlotFamily::Contour),
            PlotAction::CommitPlot,
            PlotAction::SetAxisView(AxisView::MinusZ),
        ];

        let result = execute_actions(initial, &actions);
        assert_eq!(result.intents.len(), 1);
        assert_eq!(result.intents[0].state.scalar_field, ScalarField::Density);
        assert_eq!(result.intents[0].state.plot_family, PlotFamily::Contour);
    }

    #[test]
    fn equal_plot_state_yields_equal_render_intent() {
        let mut state_a = PlotState::default();
        state_a.scalar_field = ScalarField::Pressure;
        state_a.plot_family = PlotFamily::FunctionSurface;

        let mut state_b = PlotState::default();
        state_b.scalar_field = ScalarField::Pressure;
        state_b.plot_family = PlotFamily::FunctionSurface;

        let intent_a = RenderIntent::from_state(&state_a);
        let intent_b = RenderIntent::from_state(&state_b);

        assert_eq!(intent_a, intent_b);
    }

    #[test]
    fn show_and_text_are_preserved_in_execution_result() {
        let initial = PlotState::default();
        let actions = vec![
            PlotAction::AddTextAnnotation(PlotText {
                content: "demo".to_string(),
                x: 0.1,
                y: 0.9,
            }),
            PlotAction::ShowStatus,
            PlotAction::CommitPlot,
        ];

        let result = execute_actions(initial, &actions);

        assert_eq!(result.final_state.text_annotations.len(), 1);
        assert_eq!(result.show_output.len(), 1);
        assert!(result.show_output[0].contains("SHOW:"));
        assert_eq!(result.intents.len(), 1);
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn show_without_commit_produces_status_without_render_intent() {
        let initial = PlotState::default();
        let actions = vec![PlotAction::ShowStatus];

        let result = execute_actions(initial, &actions);

        assert_eq!(
            result.intents.len(),
            0,
            "SHOW should not create a render intent"
        );
        assert_eq!(
            result.show_output.len(),
            1,
            "SHOW should still produce one status line"
        );
        assert!(
            result.show_output[0].contains("SHOW:"),
            "expected formatted SHOW output, got {:?}",
            result.show_output
        );
    }

    #[test]
    fn multiple_plot_actions_emit_multiple_intents_in_order() {
        let initial = PlotState::default();
        let actions = vec![
            PlotAction::SetScalarField(ScalarField::Density),
            PlotAction::CommitPlot,
            PlotAction::SetScalarField(ScalarField::Pressure),
            PlotAction::CommitPlot,
        ];

        let result = execute_actions(initial, &actions);
        assert_eq!(result.intents.len(), 2);
        assert_eq!(result.intents[0].state.scalar_field, ScalarField::Density);
        assert_eq!(result.intents[1].state.scalar_field, ScalarField::Pressure);
    }

    #[test]
    fn execute_parsed_script_merges_parser_and_execution_diagnostics() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("exec.com");
        fs::write(&file, "UNKNOWN_CMD\nSHOW\nPLOT/CONTOUR\n").expect("write script");

        let parsed = parse_com_file(&file).expect("parse script");
        let result = execute_parsed_script(PlotState::default(), &parsed);

        assert_eq!(result.intents.len(), 1);
        assert_eq!(result.show_output.len(), 1);
        assert!(result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("Unsupported command")));
        assert!(result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("Show status requested")));
        assert!(result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("Plot committed")));
    }

    #[test]
    fn parser_diagnostics_are_ordered_before_execution_diagnostics() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("diag_order.com");
        fs::write(&file, "UNKNOWN_CMD\nSHOW\nPLOT/CONTOUR\n").expect("write script");

        let parsed = parse_com_file(&file).expect("parse script");
        let result = execute_parsed_script(PlotState::default(), &parsed);

        assert!(
            !result.diagnostics.is_empty(),
            "expected diagnostics to be present"
        );
        assert!(
            result.diagnostics[0]
                .message
                .contains("Unsupported command"),
            "expected parser diagnostic to be first, got {:?}",
            result.diagnostics
        );
        assert!(
            result
                .diagnostics
                .iter()
                .skip(1)
                .any(|d| d.message.contains("Plot committed")),
            "expected execution diagnostics after parser diagnostics"
        );
    }
}
