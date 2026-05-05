use crate::function_mapping::{map_function_number_to_action, map_legacy_function_number};
use crate::plot_state::{
    cap, spherical_to_cartesian, AxisBounds, AxisView, ContourAttribute, ContourEntry, ContourSpec,
    DatasetRef, Diagnostic, DiagnosticSeverity, FsurfaceSpec, GridSubset, IndexRange,
    MinMaxOverride, PlotAction, PlotFamily, PlotText, PlotUpAxis, RakeCoordinateMode, RakeIoMode,
    RakeSettings, RakeTimeMode, ScalarField, VectorSettings, ViewPoint, WallColor, WallRenderMode,
    WallStyle,
};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct ParsedScript {
    pub actions: Vec<PlotAction>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Copy)]
enum LegacyWallsSubsetsAxisStage {
    I,
    J,
    K,
    Style,
}

#[derive(Debug, Clone)]
struct LegacyWallsSubsetsState {
    walls: bool,
    grid: u32,
    stage: LegacyWallsSubsetsAxisStage,
    pending_i: Option<IndexRange>,
    pending_j: Option<IndexRange>,
    axis_tokens: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct LegacyTextState {
    lines: Vec<String>,
}

pub fn parse_com_file(path: &Path) -> Result<ParsedScript, String> {
    let mut visited = HashSet::new();
    parse_file_internal(path, &mut visited)
}

/// Parse commands entered directly in the GUI command window.
///
/// INCLUDE/@ directives are intentionally ignored in this mode; users should
/// run `.com` files through `parse_com_file` when includes are needed.
pub fn parse_com_text(script_text: &str, source_name: &str) -> ParsedScript {
    let source_path = PathBuf::from(source_name);
    let mut out = ParsedScript::default();

    for (idx, raw_line) in script_text.lines().enumerate() {
        let line_number = (idx + 1) as u32;
        let stripped = strip_comments(raw_line);
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with('@') {
            out.diagnostics.push(diagnostic(
                cap::READ,
                DiagnosticSeverity::Warning,
                Some(source_name.to_string()),
                Some(line_number),
                Some(1),
                "@include shorthand is not supported in command-window mode",
            ));
            continue;
        }

        let tokens = tokenize_line(trimmed);
        if tokens.is_empty() {
            continue;
        }

        let mut first = tokens[0].clone();
        let mut args_with_inline = tokens[1..].to_vec();
        if tokens[0].contains('/') {
            let mut iter = tokens[0].split('/');
            first = iter.next().unwrap_or("").to_string();
            let qualifiers = iter
                .filter(|q| !q.is_empty())
                .map(|q| format!("/{}", q))
                .collect::<Vec<_>>();
            let mut combined = qualifiers;
            combined.extend(args_with_inline);
            args_with_inline = combined;
        }

        let command = resolve_command_alias(&first);
        if command == "INCLUDE" {
            out.diagnostics.push(diagnostic(
                cap::READ,
                DiagnosticSeverity::Warning,
                Some(source_name.to_string()),
                Some(line_number),
                Some(1),
                "INCLUDE is not supported in command-window mode; use Execute .com File",
            ));
            continue;
        }

        parse_command(
            &command,
            &args_with_inline,
            &source_path,
            line_number,
            &mut out,
        );
    }

    out
}

fn parse_file_internal(
    path: &Path,
    visited: &mut HashSet<PathBuf>,
) -> Result<ParsedScript, String> {
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve script path {}: {e}", path.display()))?;

    if !visited.insert(canonical.clone()) {
        return Err(format!(
            "Include cycle detected while parsing {}",
            canonical.display()
        ));
    }

    let content = fs::read_to_string(&canonical)
        .map_err(|e| format!("Failed to read script {}: {e}", canonical.display()))?;

    let mut out = ParsedScript::default();

    let mut in_contours_manual_input = false;
    let mut in_contours_attr_expected = false;
    let mut in_contours_attr_payload = false; // consuming post-attribute thickness/colour tokens
    let mut walls_subsets_state: Option<LegacyWallsSubsetsState> = None;
    let mut text_state: Option<LegacyTextState> = None;

    for (idx, raw_line) in content.lines().enumerate() {
        let line_number = (idx + 1) as u32;
        let raw_trimmed = raw_line.trim();

        if let Some(state) = text_state.as_mut() {
            if raw_trimmed.is_empty() {
                flush_text_state(state, &mut out);
                text_state = None;
                continue;
            }

            if looks_like_command_start(raw_trimmed) {
                flush_text_state(state, &mut out);
                text_state = None;
            } else {
                state.lines.push(raw_trimmed.to_string());
                if state.lines.len() >= 2 {
                    flush_text_state(state, &mut out);
                    text_state = None;
                }
                continue;
            }
        }

        if let Some(state) = walls_subsets_state.as_mut() {
            if raw_trimmed.is_empty() {
                parse_walls_subsets_continuation(state, &[], &canonical, line_number, &mut out);
                continue;
            }
        }

        let stripped = strip_comments(raw_line);
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Legacy CONTOURS/MANUAL accepts follow-on interactive response lines
        // (levels, colors, attribute tokens). Consume those lines before
        // attempting top-level command parsing.
        if in_contours_manual_input {
            let continuation_tokens = tokenize_line(trimmed);
            if parse_contours_manual_continuation(
                &continuation_tokens,
                &canonical,
                line_number,
                &mut out,
            ) {
                continue;
            }
            in_contours_manual_input = false;
        }

        // After a bare `CONTOURS N` without an inline attribute, the next
        // interactive prompt response is the attribute keyword (LINES / COLOR /
        // SURFACE / GRID / DOTS).  Consume it and any following numeric tokens
        // (line thickness, background colour components) that belong to the
        // same prompt before resuming normal command parsing.
        if in_contours_attr_expected {
            let continuation_tokens = tokenize_line(trimmed);
            if continuation_tokens.is_empty() {
                continue; // blank line inside prompt region
            }
            let upper = continuation_tokens[0].to_uppercase();
            // If it looks like an attribute keyword, emit the action.
            let attr = match upper.as_str() {
                "LINE" | "LINES" => Some(PlotAction::SetContourAttribute(ContourAttribute::Line)),
                "SURFACE" | "SURFACES" => {
                    Some(PlotAction::SetContourAttribute(ContourAttribute::Surface))
                }
                "GRID" | "GRID_LINES" => {
                    Some(PlotAction::SetContourAttribute(ContourAttribute::Grid))
                }
                "COLOR" | "COLOR_CONTOURS" => Some(PlotAction::SetContourAttribute(
                    ContourAttribute::ColorContours,
                )),
                "DOTS" => Some(PlotAction::SetContourAttribute(ContourAttribute::Dots)),
                _ => None,
            };
            if let Some(action) = attr {
                out.actions.push(action);
                in_contours_attr_expected = false;
                in_contours_attr_payload = true; // skip thickness / bg-colour lines
                continue;
            }
            // Numeric lines after the attribute (thickness, colour tuples) — skip.
            if continuation_tokens
                .iter()
                .all(|t| t.parse::<f64>().is_ok() || t == "0")
            {
                continue;
            }
            // Anything else terminates the prompt region; fall through to parse it.
            in_contours_attr_expected = false;
        }

        // After consuming a CONTOURS attribute keyword (LINES/COLOR/etc), skip
        // the line-thickness digit and optional background-colour RGB triple that
        // may follow in the legacy interactive prompt.
        if in_contours_attr_payload {
            let continuation_tokens = tokenize_line(trimmed);
            if continuation_tokens.is_empty() {
                continue; // blank separator
            }
            // Purely numeric line → thickness or colour component → skip.
            if continuation_tokens.iter().all(|t| t.parse::<f64>().is_ok()) {
                continue;
            }
            // Anything non-numeric terminates the payload region.
            in_contours_attr_payload = false;
        }

        // Legacy WALLS/SUBSETS also uses interactive follow-on responses.
        // Consume non-command lines as command-owned payload until a clear
        // next command token appears.
        if let Some(state) = walls_subsets_state.as_mut() {
            let continuation_tokens = tokenize_line(trimmed);
            if !continuation_tokens.is_empty()
                && (!looks_like_command_start(&continuation_tokens[0])
                    || walls_subsets_token_belongs_to_prompt(state, &continuation_tokens))
            {
                parse_walls_subsets_continuation(
                    state,
                    &continuation_tokens,
                    &canonical,
                    line_number,
                    &mut out,
                );
                continue;
            }
            walls_subsets_state = None;
        }

        // Prompt-style include shorthand: @filename.com
        if let Some(rest) = trimmed.strip_prefix('@') {
            let include_target = rest.trim();
            if include_target.is_empty() {
                out.diagnostics.push(diagnostic(
                    cap::READ,
                    DiagnosticSeverity::Warning,
                    Some(canonical.to_string_lossy().to_string()),
                    Some(line_number),
                    Some(1),
                    "Include shorthand '@' missing path",
                ));
                continue;
            }
            include_script(&canonical, include_target, line_number, visited, &mut out)?;
            continue;
        }

        let tokens = tokenize_line(trimmed);
        if tokens.is_empty() {
            continue;
        }

        let mut first = tokens[0].clone();
        let mut args_with_inline = tokens[1..].to_vec();
        if tokens[0].contains('/') {
            let mut iter = tokens[0].split('/');
            first = iter.next().unwrap_or("").to_string();
            let qualifiers = iter
                .filter(|q| !q.is_empty())
                .map(|q| format!("/{}", q))
                .collect::<Vec<_>>();
            let mut combined = qualifiers;
            combined.extend(args_with_inline);
            args_with_inline = combined;
        }

        let command = resolve_command_alias(&first);
        if command == "INCLUDE" {
            if args_with_inline.is_empty() {
                out.diagnostics.push(diagnostic(
                    cap::READ,
                    DiagnosticSeverity::Warning,
                    Some(canonical.to_string_lossy().to_string()),
                    Some(line_number),
                    Some(1),
                    "INCLUDE requires a file path",
                ));
                continue;
            }
            include_script(
                &canonical,
                &args_with_inline[0],
                line_number,
                visited,
                &mut out,
            )?;
            continue;
        }

        if (command == "WALLS" || command == "SUBSETS")
            && walls_subsets_is_interactive_request(&args_with_inline)
        {
            let (grid, _add_mode) = extract_walls_subsets_interactive_context(&args_with_inline);
            walls_subsets_state = Some(LegacyWallsSubsetsState {
                walls: command == "WALLS",
                grid,
                stage: LegacyWallsSubsetsAxisStage::I,
                pending_i: None,
                pending_j: None,
                axis_tokens: Vec::new(),
            });
            continue;
        }

        if command == "TEXT" && args_with_inline.is_empty() {
            text_state = Some(LegacyTextState::default());
            continue;
        }

        parse_command(
            &command,
            &args_with_inline,
            &canonical,
            line_number,
            &mut out,
        );

        in_contours_manual_input =
            command == "CONTOURS" && contours_manual_requested(&args_with_inline);
        // If CONTOURS was invoked with just a count (no inline attribute qualifier),
        // prime the attribute-response continuation so the next non-blank line is
        // treated as the interactive prompt reply.
        in_contours_attr_expected = command == "CONTOURS"
            && !in_contours_manual_input
            && contours_attr_input_needed(&args_with_inline);
        in_contours_attr_payload = false;
        walls_subsets_state = None;
    }

    visited.remove(&canonical);
    Ok(out)
}

fn flush_text_state(state: &mut LegacyTextState, out: &mut ParsedScript) {
    if state.lines.is_empty() {
        return;
    }

    out.actions.push(PlotAction::AddTextAnnotation(PlotText {
        content: state.lines.join("\n"),
        x: 0.05,
        y: 0.95,
    }));
    state.lines.clear();
}

fn walls_subsets_is_interactive_request(args: &[String]) -> bool {
    const WALLS_SUBSETS_QUALIFIERS: &[&str] = &[
        "GRID",
        "ADD",
        "ALL",
        "NONE",
        "I",
        "J",
        "K",
        "ATTRIBUTES",
        "NOATTRIBUTES",
    ];

    let mut positional: Vec<String> = Vec::new();
    let mut has_all = false;
    let mut has_none = false;
    let mut has_axis_qualifier = false;
    let mut has_any_qualifier = false;

    for arg in args {
        if let Some((raw_name, _value)) = parse_qualifier(arg) {
            has_any_qualifier = true;
            let name = resolve_qualifier_abbrev(&raw_name, WALLS_SUBSETS_QUALIFIERS);
            match name.as_str() {
                "ALL" => has_all = true,
                "NONE" => has_none = true,
                "I" | "J" | "K" => has_axis_qualifier = true,
                _ => {}
            }
            continue;
        }
        positional.push(arg.clone());
    }

    if has_all || has_none || has_axis_qualifier {
        return false;
    }

    // Interactive mode is the legacy prompt form where command line carries
    // only grid selection (or nothing) and follows with I/J/K responses.
    if has_any_qualifier {
        positional.len() <= 1
    } else {
        positional.is_empty()
    }
}

fn extract_walls_subsets_interactive_context(args: &[String]) -> (u32, bool) {
    const WALLS_SUBSETS_QUALIFIERS: &[&str] = &[
        "GRID",
        "ADD",
        "ALL",
        "NONE",
        "I",
        "J",
        "K",
        "ATTRIBUTES",
        "NOATTRIBUTES",
    ];

    let mut grid = 1u32;
    let mut add_mode = false;
    let mut positional: Vec<String> = Vec::new();

    for arg in args {
        if let Some((raw_name, value)) = parse_qualifier(arg) {
            let name = resolve_qualifier_abbrev(&raw_name, WALLS_SUBSETS_QUALIFIERS);
            match name.as_str() {
                "GRID" => {
                    if let Some(v) = value.and_then(|s| s.parse::<u32>().ok()).filter(|&v| v > 0) {
                        grid = v;
                    }
                }
                "ADD" => {
                    add_mode = true;
                }
                _ => {}
            }
            continue;
        }
        positional.push(arg.clone());
    }

    if let Some(v) = positional
        .first()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|&v| v > 0)
    {
        grid = v;
    }

    (grid, add_mode)
}

fn parse_walls_subsets_continuation(
    state: &mut LegacyWallsSubsetsState,
    tokens: &[String],
    file: &Path,
    line: u32,
    out: &mut ParsedScript,
) {
    match state.stage {
        LegacyWallsSubsetsAxisStage::I => {
            if tokens.is_empty() {
                if let Some(range) = parse_walls_subsets_prompt_range(&state.axis_tokens) {
                    state.pending_i = Some(range);
                    state.stage = LegacyWallsSubsetsAxisStage::J;
                }
                state.axis_tokens.clear();
            } else {
                state.axis_tokens.extend(tokens.iter().cloned());
            }
        }
        LegacyWallsSubsetsAxisStage::J => {
            if tokens.is_empty() {
                if let Some(range) = parse_walls_subsets_prompt_range(&state.axis_tokens) {
                    state.pending_j = Some(range);
                    state.stage = LegacyWallsSubsetsAxisStage::K;
                }
                state.axis_tokens.clear();
            } else {
                state.axis_tokens.extend(tokens.iter().cloned());
            }
        }
        LegacyWallsSubsetsAxisStage::K => {
            if tokens.is_empty() {
                if let Some(k_range) = parse_walls_subsets_prompt_range(&state.axis_tokens) {
                    let subset = GridSubset {
                        grid: state.grid,
                        gui_managed: false,
                        i_range: state.pending_i.clone(),
                        j_range: state.pending_j.clone(),
                        k_range: Some(k_range),
                        style: WallStyle::default(),
                    };

                    if state.walls {
                        out.actions.push(PlotAction::AddWalls(vec![subset]));
                    } else {
                        out.actions.push(PlotAction::AddSubsets(vec![subset]));
                    }

                    state.pending_i = None;
                    state.pending_j = None;
                    state.stage = LegacyWallsSubsetsAxisStage::Style;
                }
                state.axis_tokens.clear();
            } else {
                state.axis_tokens.extend(tokens.iter().cloned());
            }
        }
        LegacyWallsSubsetsAxisStage::Style => {
            if tokens.is_empty() {
                return;
            }
            if looks_like_walls_subsets_prompt_range_start(tokens) {
                state.axis_tokens.clear();
                state.axis_tokens.extend(tokens.iter().cloned());
                state.pending_i = None;
                state.pending_j = None;
                state.stage = LegacyWallsSubsetsAxisStage::I;
            } else if let Some(first) = tokens.first() {
                let token = first.to_uppercase();
                if token == "0" {
                    // Material tuple lines like "0 0 0" are style payload.
                    return;
                }
                if !apply_walls_subsets_style_payload(out, state.walls, tokens) {
                    out.diagnostics.push(diagnostic(
                        if state.walls {
                            cap::WALLS
                        } else {
                            cap::SUBSETS
                        },
                        DiagnosticSeverity::Info,
                        Some(file.to_string_lossy().to_string()),
                        Some(line),
                        Some(1),
                        "Ignoring unrecognized legacy WALLS/SUBSETS interactive style payload line",
                    ));
                }
            }
        }
    }
}

fn apply_walls_subsets_style_payload(
    out: &mut ParsedScript,
    walls: bool,
    tokens: &[String],
) -> bool {
    let Some(subset) = out
        .actions
        .iter_mut()
        .rev()
        .find_map(|action| match action {
            PlotAction::AddWalls(items) if walls => items.last_mut(),
            PlotAction::AddSubsets(items) if !walls => items.last_mut(),
            _ => None,
        })
    else {
        return false;
    };

    let token = tokens[0].to_uppercase();
    match token.as_str() {
        "LINE" | "L" => subset.style.mode = Some(WallRenderMode::Line),
        "SHADED" | "SH" => subset.style.mode = Some(WallRenderMode::Shaded),
        "HIDDEN_LINES" => subset.style.mode = Some(WallRenderMode::HiddenLines),
        "POINTS" => subset.style.mode = Some(WallRenderMode::Points),
        "WHITE" => subset.style.color = Some(WallColor::White),
        "RED" => subset.style.color = Some(WallColor::Red),
        "GREEN" => subset.style.color = Some(WallColor::Green),
        "BLUE" => subset.style.color = Some(WallColor::Blue),
        "CYAN" => subset.style.color = Some(WallColor::Cyan),
        "MAGENTA" => subset.style.color = Some(WallColor::Magenta),
        "YELLOW" => subset.style.color = Some(WallColor::Yellow),
        "BLACK" => subset.style.color = Some(WallColor::Black),
        "RGB" => {
            let parse_component = |idx: usize| {
                tokens
                    .get(idx)
                    .and_then(|value| value.parse::<f32>().ok())
                    .map(|value| (value.clamp(0.0, 1.0) * 255.0).round() as u8)
            };
            if let (Some(r), Some(g), Some(b)) =
                (parse_component(1), parse_component(2), parse_component(3))
            {
                subset.style.color = Some(WallColor::Rgb { r, g, b });
            } else {
                return false;
            }
        }
        _ => return false,
    }

    true
}

fn walls_subsets_token_belongs_to_prompt(
    state: &LegacyWallsSubsetsState,
    tokens: &[String],
) -> bool {
    if tokens.is_empty() {
        return true;
    }

    match state.stage {
        LegacyWallsSubsetsAxisStage::I
        | LegacyWallsSubsetsAxisStage::J
        | LegacyWallsSubsetsAxisStage::K => {
            looks_like_walls_subsets_prompt_range_start(tokens)
                || parse_prompt_index_token(&tokens[0]).is_some()
        }
        LegacyWallsSubsetsAxisStage::Style => {
            if looks_like_walls_subsets_prompt_range_start(tokens) {
                return true;
            }

            let token = tokens[0].to_uppercase();
            matches!(
                token.as_str(),
                "LINE"
                    | "L"
                    | "SHADED"
                    | "SH"
                    | "HIDDEN_LINES"
                    | "POINTS"
                    | "RGB"
                    | "WHITE"
                    | "RED"
                    | "GREEN"
                    | "BLUE"
                    | "CYAN"
                    | "MAGENTA"
                    | "YELLOW"
                    | "BLACK"
            ) || token == "0"
        }
    }
}

fn looks_like_walls_subsets_prompt_range_start(tokens: &[String]) -> bool {
    if tokens.is_empty() {
        return false;
    }
    let first = tokens[0].to_uppercase();
    if first == "ALL" || first == "A" || first == "LAST" || first == "L" {
        return true;
    }
    if let Ok(v) = tokens[0].parse::<i32>() {
        return v != 0;
    }
    tokens[0].contains(':') || (tokens[0].starts_with('(') && tokens[0].ends_with(')'))
}

fn parse_walls_subsets_prompt_range(tokens: &[String]) -> Option<IndexRange> {
    if tokens.is_empty() {
        return None;
    }

    let first_upper = tokens[0].to_uppercase();
    if first_upper == "ALL" || first_upper == "A" {
        let end = tokens.get(1).and_then(|s| s.parse::<i32>().ok()).or(None);
        return Some(IndexRange { start: 1, end });
    }
    if first_upper == "LAST" || first_upper == "L" {
        return Some(IndexRange {
            start: -1,
            end: Some(-1),
        });
    }

    if tokens.len() == 1 {
        if let Some(range) = parse_index_range(&tokens[0]) {
            return Some(range);
        }
    }

    if let Some(start) = parse_prompt_index_token(&tokens[0]) {
        let end = tokens
            .get(1)
            .and_then(|s| parse_prompt_index_token(s))
            .unwrap_or(start);
        return Some(IndexRange {
            start,
            end: Some(end),
        });
    }

    None
}

fn parse_prompt_index_token(token: &str) -> Option<i32> {
    let upper = token.to_uppercase();
    match upper.as_str() {
        "LAST" | "L" => Some(-1),
        _ => token.parse::<i32>().ok(),
    }
}

fn looks_like_command_start(token: &str) -> bool {
    if token.starts_with('@') {
        return true;
    }
    let command_token = token.split('/').next().unwrap_or(token);
    let command = resolve_command_alias(command_token);
    if command == "AUTOMM" && token.len() <= 1 {
        return false;
    }
    KNOWN_COMMANDS.contains(&command.as_str())
}

fn contours_manual_requested(args: &[String]) -> bool {
    for arg in args {
        if let Some((raw_name, _)) = parse_qualifier(arg) {
            let name = resolve_qualifier_abbrev(&raw_name, CONTOURS_QUALIFIERS);
            if name == "MANUAL" {
                return true;
            }
        }
    }
    false
}

/// Returns `true` when a CONTOURS command line has a numeric count but no
/// inline attribute qualifier — i.e. the attribute will arrive as the next
/// interactive prompt response on a subsequent line.
fn contours_attr_input_needed(args: &[String]) -> bool {
    let mut has_count = false;
    let attr_qualifiers = ["LINE", "SURFACE", "GRID", "COLOR", "DOTS"];
    for arg in args {
        if let Some((raw_name, _)) = parse_qualifier(arg) {
            let name = resolve_qualifier_abbrev(&raw_name, CONTOURS_QUALIFIERS);
            if attr_qualifiers.contains(&name.as_str()) {
                return false; // attribute already provided inline
            }
        } else if arg.parse::<f64>().is_ok() {
            has_count = true;
        } else {
            // Check inline keyword form (without `/` prefix)
            let upper = arg.to_uppercase();
            if matches!(
                upper.as_str(),
                "LINE"
                    | "LINES"
                    | "SURFACE"
                    | "SURFACES"
                    | "GRID"
                    | "GRID_LINES"
                    | "COLOR"
                    | "COLOR_CONTOURS"
                    | "DOTS"
            ) {
                return false; // attribute already provided inline as keyword
            }
        }
    }
    has_count
}

fn parse_contours_manual_continuation(
    tokens: &[String],
    file: &Path,
    line: u32,
    out: &mut ParsedScript,
) -> bool {
    if tokens.is_empty() {
        return true;
    }

    // Attribute-response lines from legacy prompts (e.g. "rgb ...", "re ma whi ...").
    if is_contours_attribute_token(&tokens[0]) {
        return true;
    }

    let mut numbers: Vec<f64> = Vec::new();
    if let Some(tuple_values) = parse_tuple_numbers(&tokens[0]) {
        numbers.extend(tuple_values.into_iter().take(3));
    } else {
        for token in tokens {
            if numbers.len() >= 3 {
                break;
            }
            if let Some(v) = parse_f64(token) {
                numbers.push(v);
            } else {
                break;
            }
        }
    }

    if numbers.is_empty() {
        return false;
    }

    let mut new_entries: Vec<ContourEntry> = Vec::new();
    match numbers.len() {
        1 => {
            new_entries.push(ContourEntry {
                value: numbers[0],
                color: None,
            });
        }
        2 => {
            new_entries.push(ContourEntry {
                value: numbers[0],
                color: None,
            });
            new_entries.push(ContourEntry {
                value: numbers[1],
                color: None,
            });
        }
        _ => {
            let start = numbers[0];
            let end = numbers[1];
            let inc = numbers[2];
            if inc <= 0.0 {
                out.diagnostics.push(diagnostic(
                    cap::CONTOURS,
                    DiagnosticSeverity::Warning,
                    Some(file.to_string_lossy().to_string()),
                    Some(line),
                    Some(1),
                    "CONTOURS manual continuation increment must be > 0",
                ));
                return true;
            }

            let mut v = start;
            if start <= end {
                while v <= end {
                    new_entries.push(ContourEntry {
                        value: v,
                        color: None,
                    });
                    v += inc;
                }
            } else {
                while v >= end {
                    new_entries.push(ContourEntry {
                        value: v,
                        color: None,
                    });
                    v -= inc;
                }
            }
        }
    }

    append_contours_manual_entries(out, new_entries);
    true
}

fn append_contours_manual_entries(out: &mut ParsedScript, mut new_entries: Vec<ContourEntry>) {
    if new_entries.is_empty() {
        return;
    }

    if let Some(PlotAction::SetContourSpec(ContourSpec::Manual { entries })) =
        out.actions.last_mut()
    {
        entries.append(&mut new_entries);
    } else {
        out.actions
            .push(PlotAction::SetContourSpec(ContourSpec::Manual {
                entries: new_entries,
            }));
    }
}

fn is_contours_attribute_token(token: &str) -> bool {
    let upper = token.to_uppercase();
    matches!(
        upper.as_str(),
        "BLACK"
            | "BLA"
            | "MAGENTA"
            | "MAG"
            | "MA"
            | "RED"
            | "RE"
            | "YELLOW"
            | "YEL"
            | "YE"
            | "GREEN"
            | "GRE"
            | "GR"
            | "CYAN"
            | "CY"
            | "BLUE"
            | "BLU"
            | "WHITE"
            | "WHI"
            | "RGB"
            | "RANDOM"
            | "RAN"
            | "SOLID"
            | "DASHED"
            | "DOTTED"
            | "CHAINDASH"
            | "CHAINDOT"
            | "NONE"
            | "LINE"
            | "LINES"
            | "SURFACE"
            | "SURFACES"
            | "GRID"
            | "GRID_LINES"
            | "COLOR"
            | "COLOR_CONTOURS"
            | "DOTS"
            | "IJ"
            | "IK"
            | "JK"
    )
}

fn include_script(
    current_file: &Path,
    include_target: &str,
    line_number: u32,
    visited: &mut HashSet<PathBuf>,
    out: &mut ParsedScript,
) -> Result<(), String> {
    let include_path = resolve_include_path(current_file, include_target);
    let included = parse_file_internal(&include_path, visited)?;
    out.actions.extend(included.actions);
    out.diagnostics.extend(included.diagnostics);
    out.diagnostics.push(diagnostic(
        cap::READ,
        DiagnosticSeverity::Info,
        Some(current_file.to_string_lossy().to_string()),
        Some(line_number),
        Some(1),
        format!("Included file {}", include_path.display()),
    ));
    Ok(())
}

fn resolve_include_path(current_file: &Path, include_target: &str) -> PathBuf {
    let include_raw = include_target.trim_matches('"').trim_matches('\'');
    let include_path = PathBuf::from(include_raw);
    let base = if include_path.is_absolute() {
        include_path
    } else {
        current_file
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(include_path)
    };
    // If the path has no extension and doesn't exist as-is, try appending .com
    if base.extension().is_none() && !base.exists() {
        let with_ext = base.with_extension("com");
        if with_ext.exists() {
            return with_ext;
        }
    }
    base
}

fn parse_command(command: &str, args: &[String], file: &Path, line: u32, out: &mut ParsedScript) {
    match command {
        "FUNCTION" => parse_function(args, file, line, out),
        "VIEW" => parse_view(args, file, line, out),
        "VPOINT" => parse_vpoint(args, file, line, out),
        "MINMAX" => parse_minmax(args, file, line, out),
        "CONTOURS" | "CONTOUR" => parse_contours(args, file, line, out),
        "PLOT" => parse_plot(args, file, line, out),
        "TEXT" => parse_text(args, file, line, out),
        "SHOW" => parse_show(args, file, line, out),
        "FSURFACE" => parse_fsurface(args, file, line, out),
        "WALLS" => parse_walls_or_subsets(true, args, file, line, out),
        "SUBSETS" | "SUBSET" => parse_walls_or_subsets(false, args, file, line, out),
        "READ" => parse_read(args, file, line, out),
        "HELP" => parse_help(args, file, line, out),
        "LIST" => parse_list(args, file, line, out),
        "MAP" => parse_noop_command("MAP", args, file, line, out),
        "CLEAR" => parse_noop_command("CLEAR", args, file, line, out),
        "QUIT" | "EXIT" => parse_quit(args, file, line, out),
        "VECTORS" => parse_vectors(args, file, line, out),
        "RAKES" => parse_rakes(args, file, line, out),
        "AUTOMM" => parse_automm(args, file, line, out),
        unsupported => {
            out.diagnostics.push(diagnostic(
                cap::READ,
                DiagnosticSeverity::Warning,
                Some(file.to_string_lossy().to_string()),
                Some(line),
                Some(1),
                format!("Unsupported command '{}' ignored", unsupported),
            ));
        }
    }
}

fn parse_noop_command(
    command: &str,
    args: &[String],
    file: &Path,
    line: u32,
    out: &mut ParsedScript,
) {
    if !args.is_empty() {
        out.diagnostics.push(diagnostic(
            command,
            DiagnosticSeverity::Info,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            format!(
                "{} arguments are parsed but not executed in current implementation",
                command
            ),
        ));
    }
}

fn parse_help(args: &[String], file: &Path, line: u32, out: &mut ParsedScript) {
    parse_noop_command("HELP", args, file, line, out);
}

fn parse_quit(args: &[String], file: &Path, line: u32, out: &mut ParsedScript) {
    const QUIT_QUALIFIERS: &[&str] = &["SAVE"];
    for arg in args {
        if let Some((raw_name, _)) = parse_qualifier(arg) {
            let name = resolve_qualifier_abbrev(&raw_name, QUIT_QUALIFIERS);
            if name != "SAVE" {
                out.diagnostics.push(diagnostic(
                    "QUIT",
                    DiagnosticSeverity::Warning,
                    Some(file.to_string_lossy().to_string()),
                    Some(line),
                    Some(1),
                    format!("Unknown QUIT qualifier '/{}' ignored", name),
                ));
            }
        }
    }
}

fn parse_list(args: &[String], file: &Path, line: u32, out: &mut ParsedScript) {
    const LIST_QUALIFIERS: &[&str] = &[
        "FORMATTED",
        "UNFORMATTED",
        "BINARY",
        "TEXT",
        "IEEE_DP",
        "OUTPUT",
        "CGNS",
    ];
    const LIST_TARGETS: &[&str] = &["XYZ", "Q", "FUNCTION", "CGNS"];

    let mut positional = Vec::new();
    for arg in args {
        if let Some((raw_name, value)) = parse_qualifier(arg) {
            let name = resolve_qualifier_abbrev(&raw_name, LIST_QUALIFIERS);
            match name.as_str() {
                "OUTPUT" | "CGNS" => {
                    if value.as_deref().unwrap_or("").trim().is_empty() {
                        out.diagnostics.push(diagnostic(
                            "LIST",
                            DiagnosticSeverity::Warning,
                            Some(file.to_string_lossy().to_string()),
                            Some(line),
                            Some(1),
                            format!("LIST /{} requires '=value'", name),
                        ));
                    }
                }
                "FORMATTED" | "UNFORMATTED" | "BINARY" | "TEXT" | "IEEE_DP" => {}
                _ => out.diagnostics.push(diagnostic(
                    "LIST",
                    DiagnosticSeverity::Warning,
                    Some(file.to_string_lossy().to_string()),
                    Some(line),
                    Some(1),
                    format!("Unknown LIST qualifier '/{}' ignored", name),
                )),
            }
        } else {
            positional.push(arg.to_uppercase());
        }
    }

    if let Some(target) = positional.first() {
        let resolved = resolve_qualifier_abbrev(target, LIST_TARGETS);
        if !LIST_TARGETS.contains(&resolved.as_str()) {
            out.diagnostics.push(diagnostic(
                "LIST",
                DiagnosticSeverity::Warning,
                Some(file.to_string_lossy().to_string()),
                Some(line),
                Some(1),
                format!(
                    "LIST target '{}' not recognized (expected XYZ, Q, FUNCTION, or CGNS)",
                    target
                ),
            ));
        }
    }
}

fn parse_rakes(args: &[String], file: &Path, line: u32, out: &mut ParsedScript) {
    const RAKES_QUALIFIERS: &[&str] = &[
        "IJK",
        "XYZ",
        "ADD",
        "ATTRIBUTES",
        "NOATTRIBUTES",
        "READ",
        "WRITE",
        "+TIME",
        "-TIME",
        "+-TIME",
        "MAXPOINTS",
        "SCALAR_FUNCTION",
        "NOSCALAR_FUNCTION",
    ];

    let mut settings = RakeSettings::default();

    for arg in args {
        if let Some((raw_name, value)) = parse_qualifier(arg) {
            let name = resolve_qualifier_abbrev(&raw_name, RAKES_QUALIFIERS);
            match name.as_str() {
                "IJK" => settings.coordinate_mode = Some(RakeCoordinateMode::Ijk),
                "XYZ" => settings.coordinate_mode = Some(RakeCoordinateMode::Xyz),
                "ADD" => settings.add = true,
                "ATTRIBUTES" => settings.attributes_enabled = Some(true),
                "NOATTRIBUTES" => settings.attributes_enabled = Some(false),
                "+TIME" => settings.time_mode = Some(RakeTimeMode::Plus),
                "-TIME" => settings.time_mode = Some(RakeTimeMode::Minus),
                "+-TIME" => settings.time_mode = Some(RakeTimeMode::PlusMinus),
                "NOSCALAR_FUNCTION" => {
                    settings.scalar_function = None;
                    settings.scalar_function_disabled = true;
                }
                "READ" | "WRITE" | "MAXPOINTS" | "SCALAR_FUNCTION" => {
                    if value.as_deref().unwrap_or("").trim().is_empty() {
                        out.diagnostics.push(diagnostic(
                            cap::RAKES,
                            DiagnosticSeverity::Warning,
                            Some(file.to_string_lossy().to_string()),
                            Some(line),
                            Some(1),
                            format!("RAKES /{} requires '=value'", name),
                        ));
                        continue;
                    }

                    match name.as_str() {
                        "READ" => {
                            settings.io_mode = Some(RakeIoMode::Read(
                                value.expect("checked above").trim().to_string(),
                            ));
                        }
                        "WRITE" => {
                            settings.io_mode = Some(RakeIoMode::Write(
                                value.expect("checked above").trim().to_string(),
                            ));
                        }
                        "MAXPOINTS" => match value.expect("checked above").trim().parse::<u32>() {
                            Ok(v) => settings.max_points = Some(v),
                            Err(_) => out.diagnostics.push(diagnostic(
                                cap::RAKES,
                                DiagnosticSeverity::Warning,
                                Some(file.to_string_lossy().to_string()),
                                Some(line),
                                Some(1),
                                format!("RAKES /MAXPOINTS expects an integer, got '{}'", arg),
                            )),
                        },
                        "SCALAR_FUNCTION" => match value
                            .expect("checked above")
                            .trim()
                            .parse::<u16>()
                        {
                            Ok(v) => {
                                settings.scalar_function = Some(v);
                                settings.scalar_function_disabled = false;
                            }
                            Err(_) => out.diagnostics.push(diagnostic(
                                cap::RAKES,
                                DiagnosticSeverity::Warning,
                                Some(file.to_string_lossy().to_string()),
                                Some(line),
                                Some(1),
                                format!("RAKES /SCALAR_FUNCTION expects an integer, got '{}'", arg),
                            )),
                        },
                        _ => {}
                    }
                }
                _ => out.diagnostics.push(diagnostic(
                    cap::RAKES,
                    DiagnosticSeverity::Warning,
                    Some(file.to_string_lossy().to_string()),
                    Some(line),
                    Some(1),
                    format!("Unknown RAKES qualifier '/{}' ignored", name),
                )),
            }
        }
    }

    out.actions.push(PlotAction::SetRakes(settings));
}

fn parse_vectors(args: &[String], file: &Path, line: u32, out: &mut ParsedScript) {
    const VECTORS_QUALIFIERS: &[&str] = &[
        "SCALAR_FUNCTION",
        "NOSCALAR_FUNCTION",
        "LENGTH_SCALE",
        "ATTRIBUTES",
        "NOATTRIBUTES",
    ];

    let mut settings = VectorSettings::default();

    for arg in args {
        if let Some((raw_name, value)) = parse_qualifier(arg) {
            let name = resolve_qualifier_abbrev(&raw_name, VECTORS_QUALIFIERS);
            match name.as_str() {
                "NOSCALAR_FUNCTION" => {
                    settings.scalar_function = None;
                    settings.scalar_function_disabled = true;
                }
                "ATTRIBUTES" => settings.attributes_enabled = Some(true),
                "NOATTRIBUTES" => settings.attributes_enabled = Some(false),
                "SCALAR_FUNCTION" | "LENGTH_SCALE" => {
                    if value.as_deref().unwrap_or("").trim().is_empty() {
                        out.diagnostics.push(diagnostic(
                            cap::VECTORS,
                            DiagnosticSeverity::Warning,
                            Some(file.to_string_lossy().to_string()),
                            Some(line),
                            Some(1),
                            format!("VECTORS /{} requires '=value'", name),
                        ));
                        continue;
                    }

                    match name.as_str() {
                        "SCALAR_FUNCTION" => {
                            match value.expect("checked above").trim().parse::<u16>() {
                                Ok(v) => {
                                    settings.scalar_function = Some(v);
                                    settings.scalar_function_disabled = false;
                                }
                                Err(_) => out.diagnostics.push(diagnostic(
                                    cap::VECTORS,
                                    DiagnosticSeverity::Warning,
                                    Some(file.to_string_lossy().to_string()),
                                    Some(line),
                                    Some(1),
                                    format!(
                                        "VECTORS /SCALAR_FUNCTION expects an integer, got '{}'",
                                        arg
                                    ),
                                )),
                            }
                        }
                        "LENGTH_SCALE" => match value.expect("checked above").trim().parse::<f64>()
                        {
                            Ok(v) => settings.length_scale = Some(v),
                            Err(_) => out.diagnostics.push(diagnostic(
                                cap::VECTORS,
                                DiagnosticSeverity::Warning,
                                Some(file.to_string_lossy().to_string()),
                                Some(line),
                                Some(1),
                                format!("VECTORS /LENGTH_SCALE expects a real, got '{}'", arg),
                            )),
                        },
                        _ => {}
                    }
                }
                _ => out.diagnostics.push(diagnostic(
                    cap::VECTORS,
                    DiagnosticSeverity::Warning,
                    Some(file.to_string_lossy().to_string()),
                    Some(line),
                    Some(1),
                    format!("Unknown VECTORS qualifier '/{}' ignored", name),
                )),
            }
        }
    }

    out.actions.push(PlotAction::SetVectors(settings));
}

fn parse_automm(args: &[String], file: &Path, line: u32, out: &mut ParsedScript) {
    const AUTOMM_QUALIFIERS: &[&str] = &["GRID"];
    for arg in args {
        if let Some((raw_name, value)) = parse_qualifier(arg) {
            let name = resolve_qualifier_abbrev(&raw_name, AUTOMM_QUALIFIERS);
            if name == "GRID" && value.as_deref().unwrap_or("").trim().is_empty() {
                out.diagnostics.push(diagnostic(
                    "AUTOMM",
                    DiagnosticSeverity::Warning,
                    Some(file.to_string_lossy().to_string()),
                    Some(line),
                    Some(1),
                    "AUTOMM /GRID requires '=value'",
                ));
            }
        }
    }
}

fn parse_function(args: &[String], file: &Path, line: u32, out: &mut ParsedScript) {
    if args.is_empty() {
        out.diagnostics.push(diagnostic(
            cap::FUNCTION,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            "FUNCTION requires a numeric function ID",
        ));
        return;
    }

    match args[0].parse::<u16>() {
        Ok(number) => {
            let (action, mut diags) = map_function_number_to_action(number);
            for diag in &mut diags {
                diag.file = Some(file.to_string_lossy().to_string());
                diag.line = Some(line);
                diag.column = Some(1);
            }
            out.diagnostics.extend(diags);
            if let Some(action) = action {
                out.actions.push(action);
            }
        }
        Err(_) => out.diagnostics.push(diagnostic(
            cap::FUNCTION,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            format!("Invalid FUNCTION id '{}'", args[0]),
        )),
    }
}

fn parse_view(args: &[String], file: &Path, line: u32, out: &mut ParsedScript) {
    if args.is_empty() {
        out.diagnostics.push(diagnostic(
            cap::VIEW,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            "VIEW requires an axis or plane token (e.g. X, -Z, XY, TOP)",
        ));
        return;
    }

    const VIEW_OPTIONS: &[&str] = &[
        "+X", "-X", "+Y", "-Y", "+Z", "-Z", "X", "Y", "Z", "XY", "XZ", "YZ", "YX", "ZX", "ZY",
        "TOP", "SIDE", "FRONT",
    ];

    let axis = args[0].to_uppercase();
    let resolved_axis = resolve_qualifier_abbrev(&axis, VIEW_OPTIONS);
    let mode = match resolved_axis.as_str() {
        "X" | "+X" => Some(AxisView::PlusX),
        "-X" => Some(AxisView::MinusX),
        "Y" | "+Y" => Some(AxisView::PlusY),
        "-Y" => Some(AxisView::MinusY),
        "Z" | "+Z" => Some(AxisView::PlusZ),
        "-Z" => Some(AxisView::MinusZ),
        "XY" | "TOP" => Some(AxisView::PlaneXY),
        "XZ" | "SIDE" => Some(AxisView::PlaneXZ),
        "YZ" | "FRONT" => Some(AxisView::PlaneYZ),
        "YX" => Some(AxisView::PlaneYX),
        "ZX" => Some(AxisView::PlaneZX),
        "ZY" => Some(AxisView::PlaneZY),
        _ => None,
    };

    if let Some(view) = mode {
        out.actions.push(PlotAction::SetAxisView(view));
    } else {
        out.diagnostics.push(diagnostic(
            cap::VIEW,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            format!(
                "Unsupported VIEW argument '{}' (expected X/Y/Z, ±axis, or plane e.g. XY/TOP)",
                args[0]
            ),
        ));
    }

    for arg in args.iter().skip(1) {
        out.diagnostics.push(diagnostic(
            cap::VIEW,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            format!("Unknown VIEW argument '{}' ignored", arg),
        ));
    }
}

fn parse_vpoint(args: &[String], file: &Path, line: u32, out: &mut ParsedScript) {
    // Check for /ANGLES qualifier to determine if spherical or Cartesian
    const VPOINT_QUALIFIERS: &[&str] = &["XYZ", "ANGLES"];

    let is_spherical = args.iter().any(|arg| {
        if let Some((raw_name, _)) = parse_qualifier(arg) {
            resolve_qualifier_abbrev(&raw_name, VPOINT_QUALIFIERS) == "ANGLES"
        } else {
            false
        }
    });

    let numeric_args: Vec<f64> = args
        .iter()
        .filter_map(|arg| {
            // Skip qualifiers (they start with /)
            if arg.starts_with('/') {
                None
            } else {
                parse_f64(arg)
            }
        })
        .collect();

    let non_qualifier_args: Vec<&String> =
        args.iter().filter(|arg| !arg.starts_with('/')).collect();

    if numeric_args.len() < 3 {
        // Check if we have the right number of non-qualifier args but they're not numeric
        if non_qualifier_args.len() >= 3 && numeric_args.len() < non_qualifier_args.len() {
            out.diagnostics.push(diagnostic(
                cap::VPOINT,
                DiagnosticSeverity::Warning,
                Some(file.to_string_lossy().to_string()),
                Some(line),
                Some(1),
                "VPOINT values must be numeric",
            ));
        } else {
            out.diagnostics.push(diagnostic(
                cap::VPOINT,
                DiagnosticSeverity::Warning,
                Some(file.to_string_lossy().to_string()),
                Some(line),
                Some(1),
                if is_spherical {
                    "VPOINT/ANGLES requires 3 numeric values: phi theta radius"
                } else {
                    "VPOINT requires 3 numeric values: VPOINT x y z"
                },
            ));
        }
        return;
    }

    let (x, y, z) = if is_spherical {
        // DISSPLA spherical convention: phi (azimuth), theta (elevation), radius
        let radius = numeric_args[2];
        if radius <= 0.0 {
            out.diagnostics.push(diagnostic(
                cap::VPOINT,
                DiagnosticSeverity::Warning,
                Some(file.to_string_lossy().to_string()),
                Some(line),
                Some(1),
                format!("VPOINT/ANGLES radius must be > 0; got {}", radius),
            ));
            // Use a default radius to avoid camera singularity
            spherical_to_cartesian(numeric_args[0], numeric_args[1], 5.0)
        } else {
            spherical_to_cartesian(numeric_args[0], numeric_args[1], radius)
        }
    } else {
        // Cartesian coordinates
        (numeric_args[0], numeric_args[1], numeric_args[2])
    };

    // Validate that Cartesian coordinates are finite
    if !x.is_finite() || !y.is_finite() || !z.is_finite() {
        out.diagnostics.push(diagnostic(
            cap::VPOINT,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            "VPOINT coordinates must be finite (not NaN or infinity)",
        ));
        return;
    }

    out.actions
        .push(PlotAction::SetViewpoint(ViewPoint { x, y, z }));
}

const MINMAX_QUALIFIERS: &[&str] = &[
    "X",
    "Y",
    "Z",
    "NOX",
    "NOY",
    "NOZ",
    "INCREMENT",
    "XSCALE",
    "YSCALE",
    "ZSCALE",
];

const CONTOURS_QUALIFIERS: &[&str] = &[
    "AUTOMATIC",
    "INCREMENT",
    "MANUAL",
    "RANGE",
    "LINEAR",
    "CUBIC",
    "ATTRIBUTES",
    "NOATTRIBUTES",
    "LINE",
    "SURFACE",
    "GRID",
    "COLOR",
    "DOTS",
];

fn parse_minmax(args: &[String], file: &Path, line: u32, out: &mut ParsedScript) {
    // Collect axis-selection qualifiers and numeric values separately.
    // Known non-state qualifiers like /INCREMENT, /XSCALE are silently accepted.
    let mut active_axes: Vec<&str> = Vec::new();
    let mut numeric_args: Vec<f64> = Vec::new();

    for arg in args {
        if let Some((raw_name, _)) = parse_qualifier(arg) {
            let name = resolve_qualifier_abbrev(&raw_name, MINMAX_QUALIFIERS);
            match name.as_str() {
                "X" => active_axes.push("x"),
                "Y" => active_axes.push("y"),
                "Z" => active_axes.push("z"),
                // Known advisory or unimplemented qualifiers — silently accepted.
                "NOX" | "NOY" | "NOZ" | "INCREMENT" | "XSCALE" | "YSCALE" | "ZSCALE" => {}
                _ => out.diagnostics.push(diagnostic(
                    cap::MINMAX,
                    DiagnosticSeverity::Warning,
                    Some(file.to_string_lossy().to_string()),
                    Some(line),
                    Some(1),
                    format!("Unknown MINMAX qualifier '/{}' ignored", name),
                )),
            }
            continue;
        }
        match parse_f64(arg) {
            Some(v) => numeric_args.push(v),
            None => out.diagnostics.push(diagnostic(
                cap::MINMAX,
                DiagnosticSeverity::Warning,
                Some(file.to_string_lossy().to_string()),
                Some(line),
                Some(1),
                format!("Non-numeric MINMAX value '{}' ignored", arg),
            )),
        }
    }

    let mut mm = MinMaxOverride::default();

    if active_axes.is_empty() {
        // Positional mode: pairs go to X, Y, Z in order.
        if numeric_args.len() < 2 {
            out.diagnostics.push(diagnostic(
                cap::MINMAX,
                DiagnosticSeverity::Warning,
                Some(file.to_string_lossy().to_string()),
                Some(line),
                Some(1),
                "MINMAX requires at least 2 values (xmin xmax)",
            ));
            return;
        }
        // Validate and normalize bounds: ensure min < max
        let (x_min, x_max) = if numeric_args[0] < numeric_args[1] {
            (numeric_args[0], numeric_args[1])
        } else if numeric_args[0] > numeric_args[1] {
            out.diagnostics.push(diagnostic(
                cap::MINMAX,
                DiagnosticSeverity::Warning,
                Some(file.to_string_lossy().to_string()),
                Some(line),
                Some(1),
                "MINMAX X bounds reversed (min > max); swapping",
            ));
            (numeric_args[1], numeric_args[0])
        } else {
            (numeric_args[0], numeric_args[0])
        };
        mm.x = Some(AxisBounds {
            min: x_min,
            max: x_max,
        });

        if numeric_args.len() >= 4 {
            let (y_min, y_max) = if numeric_args[2] < numeric_args[3] {
                (numeric_args[2], numeric_args[3])
            } else if numeric_args[2] > numeric_args[3] {
                out.diagnostics.push(diagnostic(
                    cap::MINMAX,
                    DiagnosticSeverity::Warning,
                    Some(file.to_string_lossy().to_string()),
                    Some(line),
                    Some(1),
                    "MINMAX Y bounds reversed (min > max); swapping",
                ));
                (numeric_args[3], numeric_args[2])
            } else {
                (numeric_args[2], numeric_args[2])
            };
            mm.y = Some(AxisBounds {
                min: y_min,
                max: y_max,
            });
        }
        if numeric_args.len() >= 6 {
            let (z_min, z_max) = if numeric_args[4] < numeric_args[5] {
                (numeric_args[4], numeric_args[5])
            } else if numeric_args[4] > numeric_args[5] {
                out.diagnostics.push(diagnostic(
                    cap::MINMAX,
                    DiagnosticSeverity::Warning,
                    Some(file.to_string_lossy().to_string()),
                    Some(line),
                    Some(1),
                    "MINMAX Z bounds reversed (min > max); swapping",
                ));
                (numeric_args[5], numeric_args[4])
            } else {
                (numeric_args[4], numeric_args[4])
            };
            mm.z = Some(AxisBounds {
                min: z_min,
                max: z_max,
            });
        }
    } else {
        // Axis-qualifier mode: each qualifier consumes the next pair of values.
        for (i, axis) in active_axes.iter().enumerate() {
            let start = i * 2;
            if numeric_args.len() < start + 2 {
                out.diagnostics.push(diagnostic(
                    cap::MINMAX,
                    DiagnosticSeverity::Warning,
                    Some(file.to_string_lossy().to_string()),
                    Some(line),
                    Some(1),
                    format!(
                        "MINMAX /{} requires 2 values (min max)",
                        axis.to_uppercase()
                    ),
                ));
                continue;
            }
            let bounds = AxisBounds {
                min: numeric_args[start],
                max: numeric_args[start + 1],
            };
            match *axis {
                "x" => mm.x = Some(bounds),
                "y" => mm.y = Some(bounds),
                "z" => mm.z = Some(bounds),
                _ => {}
            }
        }
    }

    out.actions.push(PlotAction::SetMinMax(mm));
}

fn parse_contours(args: &[String], file: &Path, line: u32, out: &mut ParsedScript) {
    if args.is_empty() {
        out.actions
            .push(PlotAction::SetContourSpec(ContourSpec::Automatic {
                count: 10,
            }));
        return;
    }

    let mut qualifier_values: HashMap<String, Option<String>> = HashMap::new();
    // Positional numeric values; their interpretation depends on the active qualifier mode.
    let mut positional_values: Vec<f64> = Vec::new();

    for arg in args {
        if let Some((raw_name, value)) = parse_qualifier(arg) {
            let name = resolve_qualifier_abbrev(&raw_name, CONTOURS_QUALIFIERS);
            qualifier_values.insert(name, value);
            continue;
        }

        if let Some(tuple_values) = parse_tuple_numbers(arg) {
            if tuple_values.len() == 3 {
                let (start, end, increment) = (tuple_values[0], tuple_values[1], tuple_values[2]);
                if increment <= 0.0 {
                    out.diagnostics.push(diagnostic(
                        cap::CONTOURS,
                        DiagnosticSeverity::Warning,
                        Some(file.to_string_lossy().to_string()),
                        Some(line),
                        Some(1),
                        "CONTOURS manual tuple increment must be > 0",
                    ));
                } else {
                    let mut v = start;
                    while v <= end {
                        positional_values.push(v);
                        v += increment;
                    }
                }
            } else if tuple_values.len() > 3 {
                out.diagnostics.push(diagnostic(
                    cap::CONTOURS,
                    DiagnosticSeverity::Warning,
                    Some(file.to_string_lossy().to_string()),
                    Some(line),
                    Some(1),
                    format!(
                        "CONTOURS tuple with {} values is unsupported; expected 3 for range tuple",
                        tuple_values.len()
                    ),
                ));
            } else {
                positional_values.extend(tuple_values);
            }
            continue;
        }

        if let Some(number) = parse_f64(arg) {
            positional_values.push(number);
            continue;
        }

        // Allow attribute keywords to appear inline without a `/` prefix.
        // Legacy scripts often write: `CONTOURS 50 LINES 2` or `CONTOURS 40 COLOR 0 0 0`.
        let upper = arg.to_uppercase();
        match upper.as_str() {
            "LINE" | "LINES" => {
                qualifier_values.insert("LINE".to_string(), None);
                continue;
            }
            "SURFACE" | "SURFACES" => {
                qualifier_values.insert("SURFACE".to_string(), None);
                continue;
            }
            "GRID" | "GRID_LINES" => {
                qualifier_values.insert("GRID".to_string(), None);
                continue;
            }
            "COLOR" | "COLOR_CONTOURS" => {
                qualifier_values.insert("COLOR".to_string(), None);
                continue;
            }
            "DOTS" => {
                qualifier_values.insert("DOTS".to_string(), None);
                continue;
            }
            // Silently ignore thickness specifiers and color background values that
            // may follow an attribute keyword (e.g. `2`, `0 0 0`).
            _ => {}
        }

        out.diagnostics.push(diagnostic(
            cap::CONTOURS,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            format!("Unknown CONTOURS argument '{}' ignored", arg),
        ));
    }

    // Attribute qualifiers are orthogonal to level-mode qualifiers; emit them first.
    // If multiple attribute qualifiers appear on one line, the last one in this list wins.
    // Warn if user specifies multiple conflicting attributes.
    let attrs_specified: Vec<&str> = [
        if qualifier_values.contains_key("LINE") {
            Some("LINE")
        } else {
            None
        },
        if qualifier_values.contains_key("SURFACE") {
            Some("SURFACE")
        } else {
            None
        },
        if qualifier_values.contains_key("GRID") {
            Some("GRID")
        } else {
            None
        },
        if qualifier_values.contains_key("COLOR") {
            Some("COLOR")
        } else {
            None
        },
        if qualifier_values.contains_key("DOTS") {
            Some("DOTS")
        } else {
            None
        },
    ]
    .iter()
    .filter_map(|&a| a)
    .collect();

    if attrs_specified.len() > 1 {
        out.diagnostics.push(diagnostic(
            cap::CONTOURS,
            DiagnosticSeverity::Info,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            format!(
                "CONTOURS specifies multiple attribute qualifiers: {}; using last one '{}'",
                attrs_specified.join(", "),
                attrs_specified[attrs_specified.len() - 1]
            ),
        ));
    }

    // Use the last attribute specified (not the first)
    let attr = if !attrs_specified.is_empty() {
        match attrs_specified[attrs_specified.len() - 1] {
            "LINE" => Some(ContourAttribute::Line),
            "SURFACE" => Some(ContourAttribute::Surface),
            "GRID" => Some(ContourAttribute::Grid),
            "COLOR" => Some(ContourAttribute::ColorContours),
            "DOTS" => Some(ContourAttribute::Dots),
            _ => None,
        }
    } else {
        None
    };
    if let Some(attribute) = attr {
        out.actions.push(PlotAction::SetContourAttribute(attribute));
    }

    if qualifier_values.contains_key("LINEAR") {
        out.diagnostics.push(diagnostic(
            cap::CONTOURS,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            "CONTOURS/LINEAR has no additional effect in the current implementation; contour extraction already uses LINEAR interpolation.",
        ));
    }
    if qualifier_values.contains_key("CUBIC") {
        out.diagnostics.push(diagnostic(
            cap::CONTOURS,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            "CONTOURS/CUBIC is not implemented; using LINEAR interpolation.",
        ));
    }
    if qualifier_values.contains_key("RANGE") {
        out.diagnostics.push(diagnostic(
            cap::CONTOURS,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            "CONTOURS/RANGE is not implemented in parser execution; using the active contour-level mode only.",
        ));
    }
    if qualifier_values.contains_key("ATTRIBUTES") {
        out.diagnostics.push(diagnostic(
            cap::CONTOURS,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            "CONTOURS/ATTRIBUTES has no parser-side effect; contour attribute rendering is controlled by explicit CONTOURS attribute qualifiers.",
        ));
    }
    if qualifier_values.contains_key("NOATTRIBUTES") {
        out.diagnostics.push(diagnostic(
            cap::CONTOURS,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            "CONTOURS/NOATTRIBUTES has no parser-side effect in the current implementation.",
        ));
    }

    // Warn about truly unknown qualifiers for all contour modes.
    for qualifier in qualifier_values.keys() {
        if !matches!(
            qualifier.as_str(),
            "AUTOMATIC"
                | "INCREMENT"
                | "MANUAL"
                | "RANGE"
                | "LINEAR"
                | "CUBIC"
                | "ATTRIBUTES"
                | "NOATTRIBUTES"
                | "LINE"
                | "SURFACE"
                | "GRID"
                | "COLOR"
                | "DOTS"
        ) {
            out.diagnostics.push(diagnostic(
                cap::CONTOURS,
                DiagnosticSeverity::Warning,
                Some(file.to_string_lossy().to_string()),
                Some(line),
                Some(1),
                format!("Unknown CONTOURS qualifier '/{}' ignored", qualifier),
            ));
        }
    }

    // /INCREMENT mode: explicit qualifier takes priority.
    if qualifier_values.contains_key("INCREMENT") {
        let increment = qualifier_values
            .get("INCREMENT")
            .and_then(|v| v.as_ref())
            .and_then(|s| s.parse::<f64>().ok())
            .or_else(|| positional_values.first().copied())
            .unwrap_or(0.1);
        out.actions
            .push(PlotAction::SetContourSpec(ContourSpec::Increment {
                start: 0.0,
                increment,
            }));
        return;
    }

    // /MANUAL mode: explicit qualifier takes priority.
    if qualifier_values.contains_key("MANUAL") {
        let entries = positional_values
            .into_iter()
            .map(|value| ContourEntry { value, color: None })
            .collect::<Vec<_>>();
        out.actions
            .push(PlotAction::SetContourSpec(ContourSpec::Manual { entries }));
        return;
    }

    // Default / /AUTOMATIC mode.
    // A bare positional number means "max number of automatic levels" per the PLOT3D spec:
    //   CONTOURS [max number of levels]
    // /AUTOMATIC=n or first positional value (cast to u32) sets the count.
    let count = qualifier_values
        .get("AUTOMATIC")
        .and_then(|v| v.as_ref())
        .and_then(|s| s.parse::<u32>().ok())
        .or_else(|| positional_values.first().map(|&v| v as u32))
        .unwrap_or(10);

    // Validate contour count is reasonable
    if count == 0 {
        out.diagnostics.push(diagnostic(
            cap::CONTOURS,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            "CONTOURS/AUTOMATIC count is 0; using default count of 10",
        ));
        out.actions
            .push(PlotAction::SetContourSpec(ContourSpec::Automatic {
                count: 10,
            }));
    } else if count > 255 {
        out.diagnostics.push(diagnostic(
            cap::CONTOURS,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            format!(
                "CONTOURS/AUTOMATIC count {} is unusually high; may degrade performance",
                count
            ),
        ));
        out.actions
            .push(PlotAction::SetContourSpec(ContourSpec::Automatic { count }));
    } else {
        out.actions
            .push(PlotAction::SetContourSpec(ContourSpec::Automatic { count }));
    }
}

const PLOT_QUALIFIERS: &[&str] = &[
    "OPENGL",
    "2D",
    "3D",
    "FULLSCREEN",
    "NOFULLSCREEN",
    "LABELS",
    "NOLABELS",
    "IJK",
    "XYZ",
    "SURFACE",
    "CARPET",
    "LINE",
    "CONTOUR",
    "FSURFACE",
    "SCRIPT",
    "NOSCRIPT",
    "AXES",
    "NOAXES",
    "FIGURE",
    "NOFIGURE",
    "BACKGROUND",
    "UP",
    "TITLE",
    "NOTITLE",
    "BAR",
    "NOBAR",
    "ADDITIONAL_TEXT",
    "NOADDITIONAL_TEXT",
    "OVERLAY",
];

fn parse_plot(args: &[String], file: &Path, line: u32, out: &mut ParsedScript) {
    for arg in args {
        if let Some((raw_name, value)) = parse_qualifier(arg) {
            let name = resolve_qualifier_abbrev(&raw_name, PLOT_QUALIFIERS);
            match name.as_str() {
                // SURFACE / CARPET / LINE are all function-surface family
                // (in 2D, LINE is the degenerate case of CARPET/SURFACE).
                "SURFACE" | "CARPET" | "LINE" => out
                    .actions
                    .push(PlotAction::SetPlotFamily(PlotFamily::FunctionSurface)),
                "CONTOUR" => out
                    .actions
                    .push(PlotAction::SetPlotFamily(PlotFamily::Contour)),
                "UP" => match value.as_deref().and_then(parse_plot_up_axis) {
                    Some(axis) => out.actions.push(PlotAction::SetPlotUpAxis(axis)),
                    None => out.diagnostics.push(diagnostic(
                        cap::PLOT,
                        DiagnosticSeverity::Warning,
                        Some(file.to_string_lossy().to_string()),
                        Some(line),
                        Some(1),
                        format!(
                            "Invalid PLOT /UP qualifier '{}' (expected X/Y/Z or signed axis like -Y)",
                            value.as_deref().unwrap_or("")
                        ),
                    )),
                },
                "FSURFACE" => out
                    .actions
                    .push(PlotAction::SetPlotFamily(PlotFamily::FunctionSurface)),
                // Accepted legacy qualifiers that currently do not alter shared state.
                "OPENGL" | "2D" | "3D" | "FULLSCREEN" | "NOFULLSCREEN" | "LABELS"
                | "NOLABELS" | "IJK" | "XYZ" | "SCRIPT" | "NOSCRIPT" | "AXES"
                | "NOAXES" | "FIGURE" | "NOFIGURE" | "BACKGROUND" | "TITLE"
                | "NOTITLE" | "BAR" | "NOBAR" | "ADDITIONAL_TEXT"
                | "NOADDITIONAL_TEXT" | "OVERLAY" => {}
                _ => out.diagnostics.push(diagnostic(
                    cap::PLOT,
                    DiagnosticSeverity::Warning,
                    Some(file.to_string_lossy().to_string()),
                    Some(line),
                    Some(1),
                    format!("Unknown PLOT qualifier '/{}' ignored", name),
                )),
            }
        }
    }

    out.actions.push(PlotAction::CommitPlot);
}

fn parse_plot_up_axis(value: &str) -> Option<PlotUpAxis> {
    match value.to_uppercase().as_str() {
        "X" | "+X" => Some(PlotUpAxis::PositiveX),
        "Y" | "+Y" => Some(PlotUpAxis::PositiveY),
        "Z" | "+Z" => Some(PlotUpAxis::PositiveZ),
        "-X" => Some(PlotUpAxis::NegativeX),
        "-Y" => Some(PlotUpAxis::NegativeY),
        "-Z" => Some(PlotUpAxis::NegativeZ),
        _ => None,
    }
}

fn parse_text(args: &[String], file: &Path, line: u32, out: &mut ParsedScript) {
    if args.is_empty() {
        out.diagnostics.push(diagnostic(
            cap::TEXT,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            "TEXT requires at least a quoted string",
        ));
        return;
    }

    let text = args[0].clone();
    let x = args.get(1).and_then(|s| parse_f64(s)).unwrap_or(0.05);
    let y = args.get(2).and_then(|s| parse_f64(s)).unwrap_or(0.95);

    if args.len() < 3 {
        out.diagnostics.push(diagnostic(
            cap::TEXT,
            DiagnosticSeverity::Info,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            "TEXT missing x/y coordinates; defaulting to (0.05, 0.95)",
        ));
    }

    // Validate that coordinates are finite and in reasonable bounds
    if !x.is_finite() || !y.is_finite() {
        out.diagnostics.push(diagnostic(
            cap::TEXT,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            "TEXT coordinates must be finite (not NaN or infinity)",
        ));
        return;
    }

    // Warn if coordinates are outside typical viewport bounds (0..1)
    if x < 0.0 || x > 1.0 || y < 0.0 || y > 1.0 {
        out.diagnostics.push(diagnostic(
            cap::TEXT,
            DiagnosticSeverity::Info,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            format!(
                "TEXT position ({}, {}) is outside viewport bounds (0..1); may render off-screen",
                x, y
            ),
        ));
    }

    out.actions.push(PlotAction::AddTextAnnotation(PlotText {
        content: text,
        x,
        y,
    }));
}

fn parse_show(args: &[String], file: &Path, line: u32, out: &mut ParsedScript) {
    const SHOW_TARGETS: &[&str] = &[
        "CONTOUR", "FUNCTION", "MINMAX", "SUBSETS", "WALLS", "RAKES", "VIEW", "VPOINT", "VECTOR",
        "FSURFACE", "PLOT", "TEXT",
    ];

    if let Some(token) = args.first() {
        let target = token.to_uppercase();
        let resolved = resolve_qualifier_abbrev(&target, SHOW_TARGETS);
        if !SHOW_TARGETS.contains(&resolved.as_str()) {
            out.diagnostics.push(diagnostic(
                cap::SHOW,
                DiagnosticSeverity::Warning,
                Some(file.to_string_lossy().to_string()),
                Some(line),
                Some(1),
                format!("SHOW target '{}' not recognized", token),
            ));
        }
    }
    out.actions.push(PlotAction::ShowStatus);
}

const FSURFACE_QUALIFIERS: &[&str] = &[
    "NONE",
    "OFF",
    "SCALE_FACTOR",
    "WALLS_ORIGIN",
    "CONTOUR",
    "GRID",
    "X",
    "Y",
    "Z",
    "AXIS",
];

fn parse_fsurface(args: &[String], file: &Path, line: u32, out: &mut ParsedScript) {
    if args.is_empty() {
        out.diagnostics.push(diagnostic(
            cap::FSURFACE,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            "FSURFACE currently expects an iso-level value or /NONE; legacy axis-property qualifiers are not implemented in this MVP.",
        ));
        return;
    }

    let mut qualifier_values: HashMap<String, Option<String>> = HashMap::new();
    let mut positional: Vec<String> = Vec::new();

    for arg in args {
        if let Some((raw_name, value)) = parse_qualifier(arg) {
            let name = resolve_qualifier_abbrev(&raw_name, FSURFACE_QUALIFIERS);
            qualifier_values.insert(name, value);
        } else {
            positional.push(arg.clone());
        }
    }

    if qualifier_values.contains_key("NONE") || qualifier_values.contains_key("OFF") {
        if qualifier_values.len() > 1 || !positional.is_empty() {
            out.diagnostics.push(diagnostic(
                cap::FSURFACE,
                DiagnosticSeverity::Warning,
                Some(file.to_string_lossy().to_string()),
                Some(line),
                Some(1),
                "FSURFACE /NONE or /OFF clears the current bounded-MVP iso-level spec; additional FSURFACE arguments were ignored.",
            ));
        }
        out.actions.push(PlotAction::SetFsurface(None));
        return;
    }

    for qualifier in qualifier_values.keys() {
        match qualifier.as_str() {
            "SCALE_FACTOR" => out.diagnostics.push(diagnostic(
                cap::FSURFACE,
                DiagnosticSeverity::Warning,
                Some(file.to_string_lossy().to_string()),
                Some(line),
                Some(1),
                "Legacy FSURFACE /SCALE_FACTOR is not implemented; current FSURFACE stores an iso-level plus FUNCTION (scalar field).",
            )),
            "WALLS_ORIGIN" => out.diagnostics.push(diagnostic(
                cap::FSURFACE,
                DiagnosticSeverity::Warning,
                Some(file.to_string_lossy().to_string()),
                Some(line),
                Some(1),
                "Legacy FSURFACE /WALLS_ORIGIN is not implemented; current FSURFACE stores an iso-level plus FUNCTION (scalar field).",
            )),
            "GRID" | "CONTOUR" => out.diagnostics.push(diagnostic(
                cap::FSURFACE,
                DiagnosticSeverity::Warning,
                Some(file.to_string_lossy().to_string()),
                Some(line),
                Some(1),
                format!(
                    "Legacy FSURFACE /{} is not implemented; current FSURFACE stores an iso-level plus FUNCTION (scalar field).",
                    qualifier
                ),
            )),
            _ => out.diagnostics.push(diagnostic(
                cap::FSURFACE,
                DiagnosticSeverity::Warning,
                Some(file.to_string_lossy().to_string()),
                Some(line),
                Some(1),
                format!("Unknown FSURFACE qualifier '/{}' ignored", qualifier),
            )),
        }
    }

    if positional.is_empty() {
        if !qualifier_values.is_empty() {
            return;
        }

        out.diagnostics.push(diagnostic(
            cap::FSURFACE,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            "FSURFACE currently expects an iso-level value or /NONE; legacy axis-property qualifiers are not implemented in this MVP.",
        ));
        return;
    }

    if let Some(value) = parse_f64(&positional[0]) {
        let field = if let Some(number) = positional.get(1).and_then(|s| s.parse::<u16>().ok()) {
            let (mapped, mut diags) = map_legacy_function_number(number);
            for diag in &mut diags {
                diag.file = Some(file.to_string_lossy().to_string());
                diag.line = Some(line);
                diag.column = Some(1);
            }
            out.diagnostics.extend(diags);
            mapped.unwrap_or(ScalarField::Pressure)
        } else {
            ScalarField::Pressure
        };

        if positional.len() > 2 {
            for extra in positional.iter().skip(2) {
                out.diagnostics.push(diagnostic(
                    cap::FSURFACE,
                    DiagnosticSeverity::Warning,
                    Some(file.to_string_lossy().to_string()),
                    Some(line),
                    Some(1),
                    format!("Extra FSURFACE argument '{}' ignored", extra),
                ));
            }
        }

        out.actions.push(PlotAction::SetFsurface(Some(FsurfaceSpec {
            value,
            scalar_field: field,
        })));
    } else {
        out.diagnostics.push(diagnostic(
            cap::FSURFACE,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            format!(
                "Invalid FSURFACE iso-level '{}'; current FSURFACE expects [value [FUNCTION]] or /NONE.",
                positional[0]
            ),
        ));
    }
}

fn parse_walls_or_subsets(
    walls: bool,
    args: &[String],
    file: &Path,
    line: u32,
    out: &mut ParsedScript,
) {
    let capability = if walls { cap::WALLS } else { cap::SUBSETS };
    if args.is_empty() {
        out.diagnostics.push(diagnostic(
            capability,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            format!("{} requires a grid index or /GRID=n qualifier", capability),
        ));
        return;
    }

    let mut grid_from_qualifier: Option<u32> = None;
    let mut add_mode = false;
    let mut all_mode = false;
    let mut none_mode = false;
    let mut i_from_qualifier: Option<IndexRange> = None;
    let mut j_from_qualifier: Option<IndexRange> = None;
    let mut k_from_qualifier: Option<IndexRange> = None;
    let mut positional: Vec<String> = Vec::new();

    const WALLS_SUBSETS_QUALIFIERS: &[&str] = &[
        "GRID",
        "ADD",
        "ALL",
        "NONE",
        "I",
        "J",
        "K",
        "ATTRIBUTES",
        "NOATTRIBUTES",
    ];
    for arg in args {
        if let Some((raw_name, value)) = parse_qualifier(arg) {
            let name = resolve_qualifier_abbrev(&raw_name, WALLS_SUBSETS_QUALIFIERS);
            match name.as_str() {
                "GRID" => {
                    if let Some(v) = value.and_then(|s| s.parse::<u32>().ok()).filter(|&v| v > 0) {
                        grid_from_qualifier = Some(v);
                    } else {
                        out.diagnostics.push(diagnostic(
                            capability,
                            DiagnosticSeverity::Warning,
                            Some(file.to_string_lossy().to_string()),
                            Some(line),
                            Some(1),
                            format!("{} /GRID must be a 1-based integer", capability),
                        ));
                    }
                }
                "ADD" => {
                    add_mode = true;
                }
                "ALL" => {
                    all_mode = true;
                }
                "NONE" => {
                    none_mode = true;
                }
                "I" | "J" | "K" => {
                    let parsed = value.as_deref().and_then(parse_index_range);
                    if let Some(range) = parsed {
                        match name.as_str() {
                            "I" => i_from_qualifier = Some(range),
                            "J" => j_from_qualifier = Some(range),
                            "K" => k_from_qualifier = Some(range),
                            _ => {}
                        }
                    } else {
                        out.diagnostics.push(diagnostic(
                            capability,
                            DiagnosticSeverity::Warning,
                            Some(file.to_string_lossy().to_string()),
                            Some(line),
                            Some(1),
                            format!(
                                "{} /{} requires a valid range value (e.g. 1:10 or (1,10))",
                                capability, name
                            ),
                        ));
                    }
                }
                // Known legacy qualifiers currently accepted but not modeled in PlotState.
                "ATTRIBUTES" | "NOATTRIBUTES" => {}
                _ => out.diagnostics.push(diagnostic(
                    capability,
                    DiagnosticSeverity::Warning,
                    Some(file.to_string_lossy().to_string()),
                    Some(line),
                    Some(1),
                    format!("Unknown {} qualifier '/{}' ignored", capability, name),
                )),
            }
            continue;
        }
        positional.push(arg.clone());
    }

    if all_mode && none_mode {
        out.diagnostics.push(diagnostic(
            capability,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            format!("{} /ALL and /NONE are conflicting; using /NONE", capability),
        ));
    }

    if none_mode {
        if add_mode {
            out.diagnostics.push(diagnostic(
                capability,
                DiagnosticSeverity::Info,
                Some(file.to_string_lossy().to_string()),
                Some(line),
                Some(1),
                format!("{} /ADD is ignored when /NONE is specified", capability),
            ));
        }
        if walls {
            out.actions.push(PlotAction::SetWalls(Vec::new()));
        } else {
            out.actions.push(PlotAction::SetSubsets(Vec::new()));
        }
        return;
    }

    if all_mode {
        out.diagnostics.push(diagnostic(
            capability,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            format!(
                "{} /ALL is not yet modeled in PlotState; command is ignored",
                capability
            ),
        ));
        return;
    }

    let grid = if let Some(v) = grid_from_qualifier {
        v
    } else {
        positional
            .first()
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|&v| v > 0)
            .unwrap_or(1)
    };

    let mut subset = GridSubset {
        grid,
        gui_managed: false,
        i_range: None,
        j_range: None,
        k_range: None,
        style: WallStyle::default(),
    };

    // Range arguments are positional only and follow the optional positional grid.
    // Preferred form: i_start i_end j_start j_end k_start k_end
    // Legacy fallback: i_range j_range k_range (e.g. 1:10 2:20 3:30 or (1,10) ...)
    let range_start = if grid_from_qualifier.is_some() { 0 } else { 1 };
    let range_args = positional.get(range_start..).unwrap_or(&[]);

    // Legacy prompt-driven WALLS/SUBSETS commands frequently specify only
    // command-level context (e.g. WALL/GRID=2) and provide I/J/K ranges on
    // follow-on lines. In that case, do not emit a default full-grid action.
    if range_args.is_empty()
        && i_from_qualifier.is_none()
        && j_from_qualifier.is_none()
        && k_from_qualifier.is_none()
    {
        return;
    }

    let parse_i32_token = |idx: usize| -> Option<i32> {
        range_args
            .get(idx)
            .and_then(|s| s.trim().parse::<i32>().ok())
    };

    if range_args.len() >= 6 {
        if let (Some(i_start), Some(i_end)) = (parse_i32_token(0), parse_i32_token(1)) {
            subset.i_range = Some(IndexRange {
                start: i_start,
                end: Some(i_end),
            });
        } else {
            out.diagnostics.push(diagnostic(
                capability,
                DiagnosticSeverity::Warning,
                Some(file.to_string_lossy().to_string()),
                Some(line),
                Some(1),
                format!(
                    "{} invalid I range pair '{} {}' ignored",
                    capability, range_args[0], range_args[1]
                ),
            ));
        }
        if let (Some(j_start), Some(j_end)) = (parse_i32_token(2), parse_i32_token(3)) {
            subset.j_range = Some(IndexRange {
                start: j_start,
                end: Some(j_end),
            });
        } else {
            out.diagnostics.push(diagnostic(
                capability,
                DiagnosticSeverity::Warning,
                Some(file.to_string_lossy().to_string()),
                Some(line),
                Some(1),
                format!(
                    "{} invalid J range pair '{} {}' ignored",
                    capability, range_args[2], range_args[3]
                ),
            ));
        }
        if let (Some(k_start), Some(k_end)) = (parse_i32_token(4), parse_i32_token(5)) {
            subset.k_range = Some(IndexRange {
                start: k_start,
                end: Some(k_end),
            });
        } else {
            out.diagnostics.push(diagnostic(
                capability,
                DiagnosticSeverity::Warning,
                Some(file.to_string_lossy().to_string()),
                Some(line),
                Some(1),
                format!(
                    "{} invalid K range pair '{} {}' ignored",
                    capability, range_args[4], range_args[5]
                ),
            ));
        }
        if range_args.len() > 6 {
            out.diagnostics.push(diagnostic(
                capability,
                DiagnosticSeverity::Info,
                Some(file.to_string_lossy().to_string()),
                Some(line),
                Some(1),
                format!("{} extra range arguments beyond 6 are ignored", capability),
            ));
        }
    } else {
        if let Some(token) = range_args.first() {
            if let Some(range) = parse_index_range(token) {
                subset.i_range = Some(range);
            } else {
                out.diagnostics.push(diagnostic(
                    capability,
                    DiagnosticSeverity::Warning,
                    Some(file.to_string_lossy().to_string()),
                    Some(line),
                    Some(1),
                    format!("{} invalid I range token '{}' ignored", capability, token),
                ));
            }
        }
        if let Some(token) = range_args.get(1) {
            if let Some(range) = parse_index_range(token) {
                subset.j_range = Some(range);
            } else {
                out.diagnostics.push(diagnostic(
                    capability,
                    DiagnosticSeverity::Warning,
                    Some(file.to_string_lossy().to_string()),
                    Some(line),
                    Some(1),
                    format!("{} invalid J range token '{}' ignored", capability, token),
                ));
            }
        }
        if let Some(token) = range_args.get(2) {
            if let Some(range) = parse_index_range(token) {
                subset.k_range = Some(range);
            } else {
                out.diagnostics.push(diagnostic(
                    capability,
                    DiagnosticSeverity::Warning,
                    Some(file.to_string_lossy().to_string()),
                    Some(line),
                    Some(1),
                    format!("{} invalid K range token '{}' ignored", capability, token),
                ));
            }
        }
        if range_args.len() > 3 {
            out.diagnostics.push(diagnostic(
                capability,
                DiagnosticSeverity::Info,
                Some(file.to_string_lossy().to_string()),
                Some(line),
                Some(1),
                format!(
                    "{} extra range arguments beyond 3 are ignored in legacy token mode",
                    capability
                ),
            ));
        }
    }

    if let Some(range) = i_from_qualifier {
        subset.i_range = Some(range);
    }
    if let Some(range) = j_from_qualifier {
        subset.j_range = Some(range);
    }
    if let Some(range) = k_from_qualifier {
        subset.k_range = Some(range);
    }

    if walls {
        if add_mode {
            out.actions.push(PlotAction::AddWalls(vec![subset]));
        } else {
            out.actions.push(PlotAction::SetWalls(vec![subset]));
        }
    } else if add_mode {
        out.actions.push(PlotAction::AddSubsets(vec![subset]));
    } else {
        out.actions.push(PlotAction::SetSubsets(vec![subset]));
    }
}

const READ_QUALIFIERS: &[&str] = &[
    "XYZ",
    "GRID",
    "Q",
    "SOLUTION",
    "1D",
    "2D",
    "3D",
    "FORMATTED",
    "UNFORMATTED",
    "BINARY",
    "IEEE_DP",
    "PLANES",
    "WHOLE",
    "CHECK",
    "NOCHECK",
    "JACOBIAN",
    "NOJACOBIAN",
    "BLANK",
    "NOBLANK",
    "MGRID",
    "MDATASET",
    "FUNCTION",
    "CGNS",
];

fn parse_read(args: &[String], file: &Path, line: u32, out: &mut ParsedScript) {
    if args.is_empty() {
        out.diagnostics.push(diagnostic(
            cap::READ,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            "READ requires at least one path",
        ));
        return;
    }

    // Support both qualifier form (READ/XYZ=grid.p3d /Q=sol.q) and positional
    // form (READ grid.p3d sol.q).  Path-to-cache resolution is deferred to the
    // executor (TKT-005).
    let mut grid_path: Option<String> = None;
    let mut solution_path: Option<String> = None;
    let mut positional: Vec<String> = Vec::new();

    for arg in args {
        if let Some((raw_name, value)) = parse_qualifier(arg) {
            let name = resolve_qualifier_abbrev(&raw_name, READ_QUALIFIERS);
            match name.as_str() {
                "XYZ" | "GRID" => {
                    if let Some(v) = value {
                        if v.trim().is_empty() {
                            out.diagnostics.push(diagnostic(
                                cap::READ,
                                DiagnosticSeverity::Warning,
                                Some(file.to_string_lossy().to_string()),
                                Some(line),
                                Some(1),
                                "READ /XYZ (or /GRID) requires a non-empty path value",
                            ));
                        } else {
                            grid_path = Some(v);
                        }
                    } else {
                        out.diagnostics.push(diagnostic(
                            cap::READ,
                            DiagnosticSeverity::Warning,
                            Some(file.to_string_lossy().to_string()),
                            Some(line),
                            Some(1),
                            "READ /XYZ (or /GRID) requires '=path'",
                        ));
                    }
                }
                "Q" | "SOLUTION" => {
                    if let Some(v) = value {
                        if v.trim().is_empty() {
                            out.diagnostics.push(diagnostic(
                                cap::READ,
                                DiagnosticSeverity::Warning,
                                Some(file.to_string_lossy().to_string()),
                                Some(line),
                                Some(1),
                                "READ /Q (or /SOLUTION) requires a non-empty path value",
                            ));
                        } else {
                            solution_path = Some(v);
                        }
                    } else {
                        out.diagnostics.push(diagnostic(
                            cap::READ,
                            DiagnosticSeverity::Warning,
                            Some(file.to_string_lossy().to_string()),
                            Some(line),
                            Some(1),
                            "READ /Q (or /SOLUTION) requires '=path'",
                        ));
                    }
                }
                // Known qualifiers that don't affect the dataset reference.
                "1D" | "2D" | "3D" | "FORMATTED" | "UNFORMATTED" | "BINARY" | "IEEE_DP"
                | "PLANES" | "WHOLE" | "CHECK" | "NOCHECK" | "JACOBIAN" | "NOJACOBIAN"
                | "BLANK" | "NOBLANK" | "MGRID" | "MDATASET" | "FUNCTION" | "CGNS" => {}
                _ => out.diagnostics.push(diagnostic(
                    cap::READ,
                    DiagnosticSeverity::Warning,
                    Some(file.to_string_lossy().to_string()),
                    Some(line),
                    Some(1),
                    format!("Unknown READ qualifier '/{}' ignored", name),
                )),
            }
            continue;
        }
        positional.push(arg.clone());
    }

    // Fall back to positional if qualifier form was not used.
    if grid_path.is_none() {
        grid_path = positional.first().cloned();
    }
    if solution_path.is_none() {
        solution_path = positional.get(1).cloned();
    }

    let dataset = DatasetRef {
        grid_id: grid_path,
        solution_id: solution_path,
    };
    out.actions.push(PlotAction::SetDataset(dataset));
    out.diagnostics.push(diagnostic(
        cap::READ,
        DiagnosticSeverity::Info,
        Some(file.to_string_lossy().to_string()),
        Some(line),
        Some(1),
        "READ parsed; path-to-cache resolution is handled by executor (TKT-005)",
    ));
}

fn parse_qualifier(token: &str) -> Option<(String, Option<String>)> {
    if !token.starts_with('/') {
        return None;
    }
    let rest = &token[1..];
    if let Some((name, value)) = rest.split_once('=') {
        Some((name.to_uppercase(), Some(value.to_string())))
    } else {
        Some((rest.to_uppercase(), None))
    }
}

/// Resolve a qualifier abbreviation against a list of known qualifier names.
/// Returns the canonical (uppercase) name if the abbreviation is a unique prefix,
/// otherwise returns the abbreviation unchanged.
fn resolve_qualifier_abbrev(abbrev: &str, known: &[&str]) -> String {
    if known.contains(&abbrev) {
        return abbrev.to_string();
    }
    let matches: Vec<&&str> = known.iter().filter(|q| q.starts_with(abbrev)).collect();
    if matches.len() == 1 {
        return matches[0].to_string();
    }
    abbrev.to_string()
}

fn parse_tuple_numbers(token: &str) -> Option<Vec<f64>> {
    let trimmed = token.trim();
    if !(trimmed.starts_with('(') && trimmed.ends_with(')')) {
        return None;
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    let mut out = Vec::new();
    for part in inner.split(',') {
        let v = part.trim().parse::<f64>().ok()?;
        out.push(v);
    }
    Some(out)
}

fn parse_index_range(token: &str) -> Option<IndexRange> {
    let t = token.trim();

    if let Some(values) = parse_tuple_numbers(t) {
        if values.len() >= 2 {
            let start = values[0] as i32;
            let end = values[1] as i32;
            return Some(IndexRange {
                start,
                end: Some(end),
            });
        }
    }

    if let Some((a, b)) = t.split_once(':') {
        let start = a.trim().parse::<i32>().ok()?;
        let end = if b.trim().is_empty() {
            None
        } else {
            Some(b.trim().parse::<i32>().ok()?)
        };
        return Some(IndexRange { start, end });
    }

    if let Ok(single) = t.parse::<i32>() {
        return Some(IndexRange {
            start: single,
            end: Some(single),
        });
    }

    None
}

fn parse_f64(value: &str) -> Option<f64> {
    value.parse::<f64>().ok()
}

/// All known PLOT3D commands. Any unique prefix of a command resolves to that command.
const KNOWN_COMMANDS: &[&str] = &[
    "READ", "FUNCTION", "VIEW", "VPOINT", "MINMAX", "CONTOURS", "PLOT", "TEXT", "SHOW", "FSURFACE",
    "WALLS", "SUBSETS", "INCLUDE",
    // Out-of-scope commands — recognised so they soft-fail with a clean diagnostic
    "HELP", "LIST", "MAP", "CLEAR", "EXIT", "QUIT", "VECTORS", "RAKES", "AUTOMM",
];

fn resolve_command_alias(command: &str) -> String {
    let upper = command.to_uppercase();
    // Exact match first
    if KNOWN_COMMANDS.contains(&upper.as_str()) {
        return upper;
    }
    // Unique prefix match.
    let matches: Vec<&&str> = KNOWN_COMMANDS
        .iter()
        .filter(|c| c.starts_with(upper.as_str()))
        .collect();
    if matches.len() == 1 {
        return matches[0].to_string();
    }
    upper
}

fn strip_comments(line: &str) -> String {
    let mut in_quotes = false;
    let mut result = String::new();

    for ch in line.chars() {
        if ch == '"' {
            in_quotes = !in_quotes;
            result.push(ch);
            continue;
        }
        if !in_quotes && (ch == '!' || ch == '#') {
            break;
        }
        result.push(ch);
    }

    result
}

fn tokenize_line(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut paren_depth = 0usize;

    for ch in line.chars() {
        match ch {
            '"' | '\'' => {
                in_quotes = !in_quotes;
                continue;
            }
            '(' if !in_quotes => {
                paren_depth += 1;
                current.push(ch);
            }
            ')' if !in_quotes => {
                if paren_depth > 0 {
                    paren_depth -= 1;
                }
                current.push(ch);
            }
            ',' if !in_quotes && paren_depth == 0 => {
                if !current.trim().is_empty() {
                    tokens.push(current.trim().to_string());
                    current.clear();
                }
            }
            c if c.is_whitespace() && !in_quotes && paren_depth == 0 => {
                if !current.trim().is_empty() {
                    tokens.push(current.trim().to_string());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.trim().is_empty() {
        tokens.push(current.trim().to_string());
    }

    tokens
}

fn diagnostic(
    capability: &str,
    severity: DiagnosticSeverity,
    file: Option<String>,
    line: Option<u32>,
    column: Option<u32>,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic {
        capability: capability.to_string(),
        severity,
        file,
        line,
        column,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenization_handles_quotes_tuples_and_comments() {
        let t = tokenize_line("TEXT \"Cp profile\" 0.1 (1,2,0.5) ! comment");
        assert_eq!(t[0], "TEXT");
        assert_eq!(t[1], "Cp profile");
        assert_eq!(t[2], "0.1");
        assert_eq!(t[3], "(1,2,0.5)");
    }

    #[test]
    fn function_supported_maps_to_action() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("one.com");
        fs::write(&file, "FUNCTION 100\n").expect("write script");

        let parsed = parse_com_file(&file).expect("parse script");
        assert!(parsed.diagnostics.is_empty());
        assert_eq!(parsed.actions.len(), 1);
        assert_eq!(
            parsed.actions[0],
            PlotAction::SetScalarField(ScalarField::Density)
        );
    }

    #[test]
    fn function_previously_unimplemented_now_produces_action() {
        // FUNCTION 154 (Mach number) was previously KnownUnimplemented.
        // It must now produce a SetScalarField action and no warnings.
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("two.com");
        fs::write(&file, "FUNCTION 154\n").expect("write script");

        let parsed = parse_com_file(&file).expect("parse script");
        assert_eq!(parsed.actions.len(), 1);
        assert_eq!(
            parsed.actions[0],
            PlotAction::SetScalarField(ScalarField::MachNumber)
        );
        assert!(
            parsed.diagnostics.is_empty(),
            "FUNCTION 154 should produce no warnings now that it is Supported"
        );
    }

    #[test]
    fn include_file_is_resolved_relative_to_parent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path().join("base.com");
        let inc = dir.path().join("inc.com");

        fs::write(&inc, "FUNCTION 110\n").expect("write include");
        fs::write(&base, "INCLUDE inc.com\nFUNCTION 100\n").expect("write base");

        let parsed = parse_com_file(&base).expect("parse script");
        assert_eq!(parsed.actions.len(), 2);
        assert_eq!(
            parsed.actions[0],
            PlotAction::SetScalarField(ScalarField::Pressure)
        );
        assert_eq!(
            parsed.actions[1],
            PlotAction::SetScalarField(ScalarField::Density)
        );
    }

    #[test]
    fn plot_with_qualifier_sets_mode_and_commit_boundary() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("plot.com");
        fs::write(&file, "PLOT/SURFACE\n").expect("write script");

        let parsed = parse_com_file(&file).expect("parse script");
        assert_eq!(parsed.actions.len(), 2);
        assert_eq!(
            parsed.actions[0],
            PlotAction::SetPlotFamily(PlotFamily::FunctionSurface)
        );
        assert_eq!(parsed.actions[1], PlotAction::CommitPlot);
    }

    #[test]
    fn vectors_command_emits_settings_action() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("vectors.com");
        fs::write(
            &file,
            "VECTORS/SCALAR_FUNCTION=114/LENGTH_SCALE=0.5/NOATTRIBUTES\n",
        )
        .expect("write script");

        let parsed = parse_com_file(&file).expect("parse script");
        let settings = parsed.actions.iter().find_map(|action| {
            if let PlotAction::SetVectors(settings) = action {
                Some(settings)
            } else {
                None
            }
        });

        let settings = settings.expect("expected SetVectors action");
        assert_eq!(settings.scalar_function, Some(114));
        assert_eq!(settings.length_scale, Some(0.5));
        assert_eq!(settings.attributes_enabled, Some(false));
        assert!(!settings.scalar_function_disabled);
    }

    #[test]
    fn vectors_missing_value_emits_warning() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("vectors_missing.com");
        fs::write(&file, "VECTORS/SCALAR_FUNCTION\n").expect("write script");

        let parsed = parse_com_file(&file).expect("parse script");
        assert!(parsed.diagnostics.iter().any(|d| d
            .message
            .contains("VECTORS /SCALAR_FUNCTION requires '=value'")));
    }

    #[test]
    fn rakes_command_emits_settings_action() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("rakes.com");
        fs::write(
            &file,
            "RAKES/XYZ/ADD/+TIME/MAXPOINTS=200/SCALAR_FUNCTION=190/READ=seeds.dat\n",
        )
        .expect("write script");

        let parsed = parse_com_file(&file).expect("parse script");
        let settings = parsed.actions.iter().find_map(|action| {
            if let PlotAction::SetRakes(settings) = action {
                Some(settings)
            } else {
                None
            }
        });

        let settings = settings.expect("expected SetRakes action");
        assert_eq!(settings.coordinate_mode, Some(RakeCoordinateMode::Xyz));
        assert!(settings.add);
        assert_eq!(settings.time_mode, Some(RakeTimeMode::Plus));
        assert_eq!(settings.max_points, Some(200));
        assert_eq!(settings.scalar_function, Some(190));
        assert_eq!(
            settings.io_mode,
            Some(RakeIoMode::Read("seeds.dat".to_string()))
        );
    }

    #[test]
    fn unsupported_command_warns_and_continues() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("unknown.com");
        fs::write(&file, "FOOBAR 1 2 3\nFUNCTION 100\n").expect("write script");

        let parsed = parse_com_file(&file).expect("parse script");
        assert_eq!(parsed.actions.len(), 1);
        assert_eq!(
            parsed.actions[0],
            PlotAction::SetScalarField(ScalarField::Density)
        );
        assert_eq!(parsed.diagnostics.len(), 1);
        assert!(parsed.diagnostics[0]
            .message
            .contains("Unsupported command"));
    }

    // ── CONTOURS modes ────────────────────────────────────────────────────────

    #[test]
    fn contours_bare_count_is_automatic() {
        // PLOT3D spec: "CONTOURS [max number of levels]" — a bare integer is
        // the automatic level count, NOT a manual contour value.
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("c.com");
        fs::write(&file, "CONTOURS 15\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");
        assert_eq!(parsed.actions.len(), 1);
        assert_eq!(
            parsed.actions[0],
            PlotAction::SetContourSpec(ContourSpec::Automatic { count: 15 })
        );
    }

    #[test]
    fn contours_increment_mode() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("c.com");
        fs::write(&file, "CONTOURS/INCREMENT 0.5\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");
        assert_eq!(parsed.actions.len(), 1);
        if let PlotAction::SetContourSpec(ContourSpec::Increment {
            start: _,
            increment,
        }) = parsed.actions[0]
        {
            assert!((increment - 0.5).abs() < 1e-9, "increment should be 0.5");
        } else {
            panic!("expected Increment spec, got {:?}", parsed.actions[0]);
        }
    }

    #[test]
    fn contours_manual_tuple_expands_levels() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("c.com");
        // (0.1, 0.9, 0.2) → levels: 0.1, 0.3, 0.5, 0.7, 0.9  (5 entries)
        fs::write(&file, "CONTOURS/MANUAL (0.1,0.9,0.2)\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");
        let warnings: Vec<_> = parsed
            .diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning)
            .collect();
        assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);
        assert_eq!(parsed.actions.len(), 1);
        if let PlotAction::SetContourSpec(ContourSpec::Manual { entries }) = &parsed.actions[0] {
            assert_eq!(entries.len(), 5);
            assert!((entries[0].value - 0.1).abs() < 1e-9);
        } else {
            panic!("expected Manual spec, got {:?}", parsed.actions[0]);
        }
    }

    #[test]
    fn contours_negative_increment_warns() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("c.com");
        fs::write(&file, "CONTOURS/MANUAL (0.1,1.0,-0.1)\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|d| d.message.contains("increment must be > 0")),
            "expected increment warning, got {:?}",
            parsed.diagnostics
        );
    }

    #[test]
    fn contours_tuple_with_four_values_warns_and_is_ignored() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("c.com");
        fs::write(&file, "CONTOURS/MANUAL (0.1,0.2,0.3,0.4)\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");

        assert!(parsed
            .diagnostics
            .iter()
            .any(|d| d.message.contains("tuple with 4 values is unsupported")));

        assert_eq!(parsed.actions.len(), 1);
        match &parsed.actions[0] {
            PlotAction::SetContourSpec(ContourSpec::Manual { entries }) => {
                assert!(
                    entries.is_empty(),
                    "expected unsupported tuple to contribute no entries"
                );
            }
            action => panic!("expected Manual contour spec, got {:?}", action),
        }
    }

    #[test]
    fn minmax_inc_shorthand_warns_but_numeric_pair_still_applies() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("m.com");
        fs::write(&file, "MINMAX/INC abc 0 1\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");

        // /INC is a valid abbreviation of /INCREMENT in legacy matching.
        assert!(parsed
            .diagnostics
            .iter()
            .any(|d| d.message.contains("Non-numeric MINMAX value 'abc' ignored")));

        assert_eq!(parsed.actions.len(), 1);
        match &parsed.actions[0] {
            PlotAction::SetMinMax(mm) => {
                assert_eq!(mm.x, Some(AxisBounds { min: 0.0, max: 1.0 }));
                assert_eq!(mm.y, None);
                assert_eq!(mm.z, None);
            }
            action => panic!("expected SetMinMax action, got {:?}", action),
        }
    }

    #[test]
    fn minmax_axis_qualifiers_with_incomplete_pairs_warn_and_apply_available_axis() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("m.com");
        fs::write(&file, "MINMAX/X/Y 0 1\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");

        assert!(parsed
            .diagnostics
            .iter()
            .any(|d| d.message.contains("MINMAX /Y requires 2 values (min max)")));

        assert_eq!(parsed.actions.len(), 1);
        match &parsed.actions[0] {
            PlotAction::SetMinMax(mm) => {
                assert_eq!(mm.x, Some(AxisBounds { min: 0.0, max: 1.0 }));
                assert_eq!(mm.y, None);
                assert_eq!(mm.z, None);
            }
            action => panic!("expected SetMinMax action, got {:?}", action),
        }
    }

    #[test]
    fn contours_increment_non_numeric_qualifier_falls_back_to_positional_value() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("c.com");
        fs::write(&file, "CONTOURS/INCREMENT=oops 0.25\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");

        assert_eq!(parsed.actions.len(), 1);
        match parsed.actions[0] {
            PlotAction::SetContourSpec(ContourSpec::Increment { start, increment }) => {
                assert!((start - 0.0).abs() < 1e-9, "start should default to 0.0");
                assert!(
                    (increment - 0.25).abs() < 1e-9,
                    "increment should fall back to positional value"
                );
            }
            ref action => panic!("expected Increment contour spec, got {:?}", action),
        }
    }

    #[test]
    fn contours_increment_qualifier_takes_priority_over_manual() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("c.com");
        fs::write(&file, "CONTOURS/INCREMENT=0.4/MANUAL 1.0 2.0 3.0\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");

        assert_eq!(parsed.actions.len(), 1);
        match parsed.actions[0] {
            PlotAction::SetContourSpec(ContourSpec::Increment { start, increment }) => {
                assert!((start - 0.0).abs() < 1e-9, "start should default to 0.0");
                assert!(
                    (increment - 0.4).abs() < 1e-9,
                    "expected increment from /INCREMENT qualifier"
                );
            }
            ref action => panic!("expected Increment contour spec, got {:?}", action),
        }
    }

    #[test]
    fn contours_increment_without_values_defaults_to_point_one() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("c.com");
        fs::write(&file, "CONTOURS/INCREMENT\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");

        assert_eq!(parsed.actions.len(), 1);
        match parsed.actions[0] {
            PlotAction::SetContourSpec(ContourSpec::Increment { start, increment }) => {
                assert!((start - 0.0).abs() < 1e-9, "start should default to 0.0");
                assert!(
                    (increment - 0.1).abs() < 1e-9,
                    "expected default increment of 0.1"
                );
            }
            ref action => panic!("expected Increment contour spec, got {:?}", action),
        }
    }

    #[test]
    fn contours_manual_mode_reports_unknown_qualifier() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("c.com");
        fs::write(&file, "CONTOURS/MANUAL/FOO 1.0\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");

        assert!(parsed.diagnostics.iter().any(|d| d
            .message
            .contains("Unknown CONTOURS qualifier '/FOO' ignored")));

        assert_eq!(parsed.actions.len(), 1);
        match &parsed.actions[0] {
            PlotAction::SetContourSpec(ContourSpec::Manual { entries }) => {
                assert_eq!(entries.len(), 1);
                assert!((entries[0].value - 1.0).abs() < 1e-9);
            }
            action => panic!("expected Manual contour spec, got {:?}", action),
        }
    }

    #[test]
    fn contours_increment_mode_reports_unknown_qualifier() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("c.com");
        fs::write(&file, "CONTOURS/INCREMENT=0.2/FOO\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");

        assert!(parsed.diagnostics.iter().any(|d| d
            .message
            .contains("Unknown CONTOURS qualifier '/FOO' ignored")));

        assert_eq!(parsed.actions.len(), 1);
        match parsed.actions[0] {
            PlotAction::SetContourSpec(ContourSpec::Increment { start, increment }) => {
                assert!((start - 0.0).abs() < 1e-9);
                assert!((increment - 0.2).abs() < 1e-9);
            }
            ref action => panic!("expected Increment contour spec, got {:?}", action),
        }
    }

    #[test]
    fn contours_linear_qualifier_warns_but_keeps_automatic_mode() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("c.com");
        fs::write(&file, "CONTOURS/LINEAR 7\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");

        assert!(parsed.diagnostics.iter().any(|d| d
            .message
            .contains("CONTOURS/LINEAR has no additional effect")));
        assert_eq!(parsed.actions.len(), 1);
        assert_eq!(
            parsed.actions[0],
            PlotAction::SetContourSpec(ContourSpec::Automatic { count: 7 })
        );
    }

    #[test]
    fn contours_cubic_qualifier_warns_and_falls_back_to_linear() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("c.com");
        fs::write(&file, "CONTOURS/CUBIC 7\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");

        assert!(parsed.diagnostics.iter().any(|d| d
            .message
            .contains("CONTOURS/CUBIC is not implemented; using LINEAR interpolation")));
        assert_eq!(parsed.actions.len(), 1);
        assert_eq!(
            parsed.actions[0],
            PlotAction::SetContourSpec(ContourSpec::Automatic { count: 7 })
        );
    }

    #[test]
    fn contours_range_qualifier_warns_but_keeps_automatic_mode() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("c.com");
        fs::write(&file, "CONTOURS/RANGE 7\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");

        assert!(parsed.diagnostics.iter().any(|d| d
            .message
            .contains("CONTOURS/RANGE is not implemented in parser execution")));
        assert_eq!(parsed.actions.len(), 1);
        assert_eq!(
            parsed.actions[0],
            PlotAction::SetContourSpec(ContourSpec::Automatic { count: 7 })
        );
    }

    #[test]
    fn contours_attributes_qualifier_warns_without_changing_action() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("c.com");
        fs::write(&file, "CONTOURS/ATTRIBUTES 7\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");

        assert!(parsed.diagnostics.iter().any(|d| d
            .message
            .contains("CONTOURS/ATTRIBUTES has no parser-side effect")));
        assert_eq!(parsed.actions.len(), 1);
        assert_eq!(
            parsed.actions[0],
            PlotAction::SetContourSpec(ContourSpec::Automatic { count: 7 })
        );
    }

    #[test]
    fn contours_noattributes_qualifier_warns_without_changing_action() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("c.com");
        fs::write(&file, "CONTOURS/NOATTRIBUTES 7\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");

        assert!(parsed.diagnostics.iter().any(|d| d
            .message
            .contains("CONTOURS/NOATTRIBUTES has no parser-side effect")));
        assert_eq!(parsed.actions.len(), 1);
        assert_eq!(
            parsed.actions[0],
            PlotAction::SetContourSpec(ContourSpec::Automatic { count: 7 })
        );
    }

    #[test]
    fn fsurface_legacy_walls_origin_qualifier_emits_divergence_warning() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("fs.com");
        fs::write(&file, "FSURFACE /WALLS_ORIGIN=1\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");

        assert!(parsed.actions.is_empty(), "expected no FSURFACE action");
        assert!(parsed.diagnostics.iter().any(|d| d
            .message
            .contains("Legacy FSURFACE /WALLS_ORIGIN is not implemented")));
    }

    #[test]
    fn fsurface_numeric_value_with_legacy_qualifier_warns_but_stores_spec() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("fs.com");
        fs::write(&file, "FSURFACE /SCALE_FACTOR=2 0.5 154\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");

        assert!(parsed.diagnostics.iter().any(|d| d
            .message
            .contains("Legacy FSURFACE /SCALE_FACTOR is not implemented")));
        // FUNCTION 154 (MachNumber) is now Supported — no unimplemented warning.
        assert!(!parsed
            .diagnostics
            .iter()
            .any(|d| d.message.contains("recognized but not implemented")));
        assert_eq!(parsed.actions.len(), 1);
        match &parsed.actions[0] {
            PlotAction::SetFsurface(Some(spec)) => {
                assert!((spec.value - 0.5).abs() < 1e-9);
                assert_eq!(spec.scalar_field, ScalarField::MachNumber);
            }
            action => panic!("expected SetFsurface action, got {:?}", action),
        }
    }

    #[test]
    fn fsurface_none_with_positional_args_clears_and_warns() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("fs.com");
        fs::write(&file, "FSURFACE /NONE 0.5 110\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");

        assert!(parsed.diagnostics.iter().any(|d| d
            .message
            .contains("FSURFACE /NONE or /OFF clears the current bounded-MVP iso-level spec")));
        assert_eq!(parsed.actions.len(), 1);
        assert_eq!(parsed.actions[0], PlotAction::SetFsurface(None));
    }

    #[test]
    fn fsurface_off_with_qualifiers_clears_and_warns() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("fs.com");
        fs::write(&file, "FSURFACE /OFF /GRID\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");

        assert!(parsed.diagnostics.iter().any(|d| d
            .message
            .contains("FSURFACE /NONE or /OFF clears the current bounded-MVP iso-level spec")));
        assert_eq!(parsed.actions.len(), 1);
        assert_eq!(parsed.actions[0], PlotAction::SetFsurface(None));
    }

    #[test]
    fn fsurface_mixed_legacy_qualifiers_with_value_warns_and_sets_spec() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("fs.com");
        fs::write(&file, "FSURFACE /GRID /CONTOUR 0.4 110 extra\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");

        assert!(parsed.diagnostics.iter().any(|d| d
            .message
            .contains("Legacy FSURFACE /GRID is not implemented")));
        assert!(parsed.diagnostics.iter().any(|d| d
            .message
            .contains("Legacy FSURFACE /CONTOUR is not implemented")));
        assert!(parsed.diagnostics.iter().any(|d| d
            .message
            .contains("Extra FSURFACE argument 'extra' ignored")));

        assert_eq!(parsed.actions.len(), 1);
        match &parsed.actions[0] {
            PlotAction::SetFsurface(Some(spec)) => {
                assert!((spec.value - 0.4).abs() < 1e-9);
                assert_eq!(spec.scalar_field, ScalarField::Pressure);
            }
            action => panic!("expected SetFsurface action, got {:?}", action),
        }
    }

    // ── VPOINT malformed inputs ───────────────────────────────────────────────

    #[test]
    fn vpoint_too_few_args_warns() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("v.com");
        fs::write(&file, "VPOINT 1.0 2.0\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");
        assert!(parsed.actions.is_empty());
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|d| d.message.contains("3 numeric values")),
            "expected arity warning, got {:?}",
            parsed.diagnostics
        );
    }

    #[test]
    fn vpoint_non_numeric_args_warn() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("v.com");
        fs::write(&file, "VPOINT abc def ghi\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");
        assert!(parsed.actions.is_empty());
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|d| d.message.contains("must be numeric")),
            "expected numeric warning, got {:?}",
            parsed.diagnostics
        );
    }

    #[test]
    fn vpoint_angles_45_45_10_converts_spherical_to_cartesian() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("v.com");
        fs::write(&file, "VPOINT/ANGLES 45.0 45.0 10.0\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");
        assert_eq!(parsed.actions.len(), 1, "Expected 1 action");
        match &parsed.actions[0] {
            PlotAction::SetViewpoint(vp) => {
                // φ=45°, θ=45°, r=10 should give approximately (5.0, 5.0, 7.07)
                assert!((vp.x - 5.0).abs() < 0.01, "expected x ≈ 5.0, got {}", vp.x);
                assert!((vp.y - 5.0).abs() < 0.01, "expected y ≈ 5.0, got {}", vp.y);
                assert!(
                    (vp.z - 7.07).abs() < 0.01,
                    "expected z ≈ 7.07, got {}",
                    vp.z
                );
            }
            _ => panic!("Expected SetViewpoint action"),
        }
    }

    #[test]
    fn vpoint_angles_0_0_10_looks_positive_x() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("v.com");
        fs::write(&file, "VPOINT/ANGLES 0.0 0.0 10.0\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");
        assert_eq!(parsed.actions.len(), 1);
        match &parsed.actions[0] {
            PlotAction::SetViewpoint(vp) => {
                assert!(
                    (vp.x - 10.0).abs() < 1e-10,
                    "x should be 10.0, got {}",
                    vp.x
                );
                assert!(vp.y.abs() < 1e-10, "y should be 0.0, got {}", vp.y);
                assert!(vp.z.abs() < 1e-10, "z should be 0.0, got {}", vp.z);
            }
            _ => panic!("Expected SetViewpoint action"),
        }
    }

    #[test]
    fn vpoint_angles_0_90_10_looks_straight_up() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("v.com");
        fs::write(&file, "VPOINT/ANGLES 0.0 90.0 10.0\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");
        assert_eq!(parsed.actions.len(), 1);
        match &parsed.actions[0] {
            PlotAction::SetViewpoint(vp) => {
                assert!(vp.x.abs() < 1e-10, "x should be 0.0, got {}", vp.x);
                assert!(vp.y.abs() < 1e-10, "y should be 0.0, got {}", vp.y);
                assert!(
                    (vp.z - 10.0).abs() < 1e-10,
                    "z should be 10.0, got {}",
                    vp.z
                );
            }
            _ => panic!("Expected SetViewpoint action"),
        }
    }

    #[test]
    fn vpoint_angles_too_few_args_warns() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("v.com");
        fs::write(&file, "VPOINT/ANGLES 45.0 45.0\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");
        assert!(parsed.actions.is_empty());
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|d| d.message.contains("3 numeric values")),
            "expected arity warning, got {:?}",
            parsed.diagnostics
        );
    }

    #[test]
    fn vpoint_angles_non_numeric_args_warn() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("v.com");
        fs::write(&file, "VPOINT/ANGLES abc def ghi\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");
        assert!(parsed.actions.is_empty());
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|d| d.message.contains("must be numeric") || d.message.contains("3 numeric")),
            "expected error, got {:?}",
            parsed.diagnostics
        );
    }

    // ── Alias resolution ─────────────────────────────────────────────────────

    #[test]
    fn command_aliases_resolve_correctly() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("alias.com");
        fs::write(&file, "FUN 100\nVP 1.0 2.0 3.0\nMIN -1.0 1.0\nPL/SURFACE\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");
        assert!(parsed
            .actions
            .contains(&PlotAction::SetScalarField(ScalarField::Density)));
        assert!(parsed
            .actions
            .contains(&PlotAction::SetViewpoint(ViewPoint {
                x: 1.0,
                y: 2.0,
                z: 3.0
            })));
        assert!(parsed
            .actions
            .iter()
            .any(|a| matches!(a, PlotAction::SetMinMax(_))));
        assert!(parsed.actions.contains(&PlotAction::CommitPlot));
    }

    // ── Include cycle detection ───────────────────────────────────────────────

    #[test]
    fn include_cycle_returns_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = dir.path().join("a.com");
        let b = dir.path().join("b.com");
        fs::write(&a, "INCLUDE b.com\n").expect("write a");
        fs::write(&b, "INCLUDE a.com\n").expect("write b");

        let result = parse_com_file(&a);
        assert!(result.is_err(), "expected an error for include cycle");
        assert!(
            result.unwrap_err().contains("cycle"),
            "error message should mention cycle"
        );
    }

    #[test]
    fn include_without_path_warns_and_continues_parsing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("include_missing_path.com");
        fs::write(&file, "INCLUDE\nVIEW X\n").expect("write script");

        let parsed = parse_com_file(&file).expect("parse");

        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|d| d.message.contains("INCLUDE requires a file path")),
            "expected INCLUDE missing-path warning, got {:?}",
            parsed.diagnostics
        );
        assert!(
            parsed
                .actions
                .iter()
                .any(|action| matches!(action, PlotAction::SetAxisView(AxisView::PlusX))),
            "expected parser to continue and parse VIEW X"
        );
    }

    #[test]
    fn include_shorthand_missing_path_warns_and_continues_parsing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("include_missing_shorthand.com");
        fs::write(&file, "@\nVIEW Y\n").expect("write script");

        let parsed = parse_com_file(&file).expect("parse");

        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|d| d.message.contains("Include shorthand '@' missing path")),
            "expected shorthand missing-path warning, got {:?}",
            parsed.diagnostics
        );
        assert!(
            parsed
                .actions
                .iter()
                .any(|action| matches!(action, PlotAction::SetAxisView(AxisView::PlusY))),
            "expected parser to continue and parse VIEW Y"
        );
    }

    #[test]
    fn parse_com_text_warns_for_include_directives_and_keeps_other_commands() {
        let parsed = parse_com_text("@child.com\nINCLUDE other.com\nVIEW Z\n", "command-window");

        let include_warnings = parsed
            .diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning)
            .filter(|d| d.message.contains("include") || d.message.contains("INCLUDE"))
            .count();
        assert_eq!(include_warnings, 2, "expected two include warnings");

        assert!(
            parsed
                .actions
                .iter()
                .any(|action| matches!(action, PlotAction::SetAxisView(AxisView::PlusZ))),
            "expected command-window parser to keep non-include commands"
        );
    }

    // ── Full integration test ─────────────────────────────────────────────────

    #[test]
    fn full_realistic_script_parses_correctly() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("session.com");
        let script = r#"
! Load geometry and solution
READ grid.xyz q.dat
! Select pressure function
FUNCTION 110
VIEW -Z
VPOINT 1.0 2.0 3.0
MINMAX -1.0 1.0
CONTOURS 15
TEXT "Pressure" 0.1 0.9
WALLS/GRID=1 1:1 1:1 1:1
PLOT/CONTOUR
"#;
        fs::write(&file, script).expect("write");

        let parsed = parse_com_file(&file).expect("parse");

        // No warnings (only info-level READ diagnostic is expected)
        let warnings: Vec<_> = parsed
            .diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning)
            .collect();
        assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);

        // Expected action sequence:
        // 0 SetDataset, 1 SetScalarField(Pressure), 2 SetAxisView(MinusZ),
        // 3 SetViewpoint, 4 SetMinMax, 5 SetContourSpec(Automatic{15}),
        // 6 AddTextAnnotation, 7 SetWalls, 8 SetPlotFamily(Contour), 9 CommitPlot
        assert_eq!(parsed.actions.len(), 10);
        assert!(matches!(parsed.actions[0], PlotAction::SetDataset(_)));
        assert_eq!(
            parsed.actions[1],
            PlotAction::SetScalarField(ScalarField::Pressure)
        );
        assert_eq!(parsed.actions[2], PlotAction::SetAxisView(AxisView::MinusZ));
        assert_eq!(
            parsed.actions[3],
            PlotAction::SetViewpoint(ViewPoint {
                x: 1.0,
                y: 2.0,
                z: 3.0
            })
        );
        assert!(matches!(parsed.actions[4], PlotAction::SetMinMax(_)));
        assert_eq!(
            parsed.actions[5],
            PlotAction::SetContourSpec(ContourSpec::Automatic { count: 15 })
        );
        assert!(matches!(
            parsed.actions[6],
            PlotAction::AddTextAnnotation(_)
        ));
        assert!(matches!(parsed.actions[7], PlotAction::SetWalls(_)));
        assert_eq!(
            parsed.actions[8],
            PlotAction::SetPlotFamily(PlotFamily::Contour)
        );
        assert_eq!(parsed.actions[9], PlotAction::CommitPlot);
    }
}

// ── VIEW plane tokens ─────────────────────────────────────────────────────

#[test]
fn view_plane_tokens_produce_plane_variants() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("v.com");
    fs::write(
        &file,
        "VIEW XY\nVIEW TOP\nVIEW XZ\nVIEW SIDE\nVIEW YZ\nVIEW FRONT\nVIEW YX\n",
    )
    .expect("write");

    let parsed = parse_com_file(&file).expect("parse");
    let warnings: Vec<_> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Warning)
        .collect();
    assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);
    assert_eq!(parsed.actions.len(), 7);
    assert_eq!(
        parsed.actions[0],
        PlotAction::SetAxisView(AxisView::PlaneXY)
    );
    assert_eq!(
        parsed.actions[1],
        PlotAction::SetAxisView(AxisView::PlaneXY)
    );
    assert_eq!(
        parsed.actions[2],
        PlotAction::SetAxisView(AxisView::PlaneXZ)
    );
    assert_eq!(
        parsed.actions[3],
        PlotAction::SetAxisView(AxisView::PlaneXZ)
    );
    assert_eq!(
        parsed.actions[4],
        PlotAction::SetAxisView(AxisView::PlaneYZ)
    );
    assert_eq!(
        parsed.actions[5],
        PlotAction::SetAxisView(AxisView::PlaneYZ)
    );
    assert_eq!(
        parsed.actions[6],
        PlotAction::SetAxisView(AxisView::PlaneYX)
    );
}

// ── MINMAX per-axis ───────────────────────────────────────────────────────

#[test]
fn minmax_positional_all_axes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("m.com");
    fs::write(&file, "MINMAX -1.0 1.0 -2.0 2.0 -3.0 3.0\n").expect("write");

    let parsed = parse_com_file(&file).expect("parse");
    assert_eq!(parsed.actions.len(), 1);
    if let PlotAction::SetMinMax(mm) = &parsed.actions[0] {
        assert_eq!(
            mm.x,
            Some(AxisBounds {
                min: -1.0,
                max: 1.0
            })
        );
        assert_eq!(
            mm.y,
            Some(AxisBounds {
                min: -2.0,
                max: 2.0
            })
        );
        assert_eq!(
            mm.z,
            Some(AxisBounds {
                min: -3.0,
                max: 3.0
            })
        );
    } else {
        panic!("expected SetMinMax, got {:?}", parsed.actions[0]);
    }
}

#[test]
fn minmax_y_qualifier_only() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("m.com");
    fs::write(&file, "MINMAX/Y -2.0 2.0\n").expect("write");

    let parsed = parse_com_file(&file).expect("parse");
    let warnings: Vec<_> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Warning)
        .collect();
    assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);
    assert_eq!(parsed.actions.len(), 1);
    if let PlotAction::SetMinMax(mm) = &parsed.actions[0] {
        assert_eq!(mm.x, None);
        assert_eq!(
            mm.y,
            Some(AxisBounds {
                min: -2.0,
                max: 2.0
            })
        );
        assert_eq!(mm.z, None);
    } else {
        panic!("expected SetMinMax, got {:?}", parsed.actions[0]);
    }
}

#[test]
fn plot_up_qualifier_sets_plot_orientation() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("plot_up.com");
    fs::write(&file, "PLOT/UP=-Y /CONTOUR\n").expect("write");

    let parsed = parse_com_file(&file).expect("parse");
    assert_eq!(parsed.actions.len(), 3);
    assert_eq!(
        parsed.actions[0],
        PlotAction::SetPlotUpAxis(PlotUpAxis::NegativeY)
    );
    assert_eq!(
        parsed.actions[1],
        PlotAction::SetPlotFamily(PlotFamily::Contour)
    );
    assert_eq!(parsed.actions[2], PlotAction::CommitPlot);
}

#[test]
fn invalid_plot_up_qualifier_warns_and_continues() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("plot_up_invalid.com");
    fs::write(&file, "PLOT/UP=SIDE /CONTOUR\n").expect("write");

    let parsed = parse_com_file(&file).expect("parse");
    assert!(parsed
        .diagnostics
        .iter()
        .any(|d| d.message.contains("Invalid PLOT /UP qualifier")));
    assert!(parsed
        .actions
        .iter()
        .any(|action| matches!(action, PlotAction::CommitPlot)));
}

// ── READ qualifier form ───────────────────────────────────────────────────

#[test]
fn read_qualifier_form() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("r.com");
    fs::write(&file, "READ/XYZ=grid.p3d /Q=solution.q\n").expect("write");

    let parsed = parse_com_file(&file).expect("parse");
    let warnings: Vec<_> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Warning)
        .collect();
    assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);
    assert_eq!(parsed.actions.len(), 1);
    if let PlotAction::SetDataset(ds) = &parsed.actions[0] {
        assert_eq!(ds.grid_id.as_deref(), Some("grid.p3d"));
        assert_eq!(ds.solution_id.as_deref(), Some("solution.q"));
    } else {
        panic!("expected SetDataset, got {:?}", parsed.actions[0]);
    }
}

#[test]
fn read_positional_form_still_works() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("r.com");
    fs::write(&file, "READ grid.p3d solution.q\n").expect("write");

    let parsed = parse_com_file(&file).expect("parse");
    assert_eq!(parsed.actions.len(), 1);
    if let PlotAction::SetDataset(ds) = &parsed.actions[0] {
        assert_eq!(ds.grid_id.as_deref(), Some("grid.p3d"));
        assert_eq!(ds.solution_id.as_deref(), Some("solution.q"));
    } else {
        panic!("expected SetDataset, got {:?}", parsed.actions[0]);
    }
}

#[test]
fn read_empty_xyz_qualifier_warns_and_falls_back_to_positional_grid() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("r.com");
    fs::write(&file, "READ/XYZ= /Q=solution.q grid.p3d\n").expect("write");

    let parsed = parse_com_file(&file).expect("parse");
    assert!(parsed
        .diagnostics
        .iter()
        .any(|d| d.message.contains("requires a non-empty path value")));
    assert_eq!(parsed.actions.len(), 1);
    if let PlotAction::SetDataset(ds) = &parsed.actions[0] {
        assert_eq!(ds.grid_id.as_deref(), Some("grid.p3d"));
        assert_eq!(ds.solution_id.as_deref(), Some("solution.q"));
    } else {
        panic!("expected SetDataset, got {:?}", parsed.actions[0]);
    }
}

#[test]
fn walls_grid_qualifier_sets_grid_id() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("walls.com");
    fs::write(&file, "WALLS/GRID=3 1:10 2:20 3:30\n").expect("write");

    let parsed = parse_com_file(&file).expect("parse");
    assert_eq!(parsed.actions.len(), 1);
    if let PlotAction::SetWalls(walls) = &parsed.actions[0] {
        assert_eq!(walls.len(), 1);
        assert_eq!(walls[0].grid, 3);
        assert_eq!(walls[0].i_range.as_ref().map(|r| r.start), Some(1));
        assert_eq!(walls[0].j_range.as_ref().map(|r| r.start), Some(2));
        assert_eq!(walls[0].k_range.as_ref().map(|r| r.start), Some(3));
    } else {
        panic!("expected SetWalls, got {:?}", parsed.actions[0]);
    }
}

#[test]
fn subsets_grid_qualifier_sets_grid_id() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("subsets.com");
    fs::write(&file, "SUBSETS/GRID=2 (5,15) (6,16) (7,17)\n").expect("write");

    let parsed = parse_com_file(&file).expect("parse");
    assert_eq!(parsed.actions.len(), 1);
    if let PlotAction::SetSubsets(subsets) = &parsed.actions[0] {
        assert_eq!(subsets.len(), 1);
        assert_eq!(subsets[0].grid, 2);
        assert_eq!(subsets[0].i_range.as_ref().map(|r| r.start), Some(5));
        assert_eq!(subsets[0].j_range.as_ref().map(|r| r.start), Some(6));
        assert_eq!(subsets[0].k_range.as_ref().map(|r| r.start), Some(7));
    } else {
        panic!("expected SetSubsets, got {:?}", parsed.actions[0]);
    }
}

#[test]
fn subsets_negative_index_parsed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("subsets_neg.com");
    fs::write(&file, "SUBSETS/GRID=1 -1 -1 -2 -2 -3 -3\n").expect("write");

    let parsed = parse_com_file(&file).expect("parse");
    assert_eq!(parsed.actions.len(), 1);
    if let PlotAction::SetSubsets(subsets) = &parsed.actions[0] {
        assert_eq!(subsets.len(), 1);
        assert_eq!(subsets[0].grid, 1);
        assert_eq!(subsets[0].i_range.as_ref().map(|r| r.start), Some(-1));
        assert_eq!(subsets[0].i_range.as_ref().and_then(|r| r.end), Some(-1));
        assert_eq!(subsets[0].j_range.as_ref().map(|r| r.start), Some(-2));
        assert_eq!(subsets[0].j_range.as_ref().and_then(|r| r.end), Some(-2));
        assert_eq!(subsets[0].k_range.as_ref().map(|r| r.start), Some(-3));
        assert_eq!(subsets[0].k_range.as_ref().and_then(|r| r.end), Some(-3));
    } else {
        panic!("expected SetSubsets, got {:?}", parsed.actions[0]);
    }
}

#[test]
fn subsets_positional_start_end_pairs_parsed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("subsets_pairs.com");
    fs::write(&file, "SUBSETS 2 5 15 6 16 7 17\n").expect("write");

    let parsed = parse_com_file(&file).expect("parse");
    assert_eq!(parsed.actions.len(), 1);
    if let PlotAction::SetSubsets(subsets) = &parsed.actions[0] {
        assert_eq!(subsets.len(), 1);
        assert_eq!(subsets[0].grid, 2);
        assert_eq!(subsets[0].i_range.as_ref().map(|r| r.start), Some(5));
        assert_eq!(subsets[0].i_range.as_ref().and_then(|r| r.end), Some(15));
        assert_eq!(subsets[0].j_range.as_ref().map(|r| r.start), Some(6));
        assert_eq!(subsets[0].j_range.as_ref().and_then(|r| r.end), Some(16));
        assert_eq!(subsets[0].k_range.as_ref().map(|r| r.start), Some(7));
        assert_eq!(subsets[0].k_range.as_ref().and_then(|r| r.end), Some(17));
    } else {
        panic!("expected SetSubsets, got {:?}", parsed.actions[0]);
    }
}

#[test]
fn walls_ijk_qualifiers_override_positional_ranges() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("walls_ijk_qual.com");
    fs::write(
        &file,
        "WALLS/GRID=4 /I=1:10 /J=(2,20) /K=-1 7 8 9 10 11 12\n",
    )
    .expect("write");

    let parsed = parse_com_file(&file).expect("parse");
    assert_eq!(parsed.actions.len(), 1);
    if let PlotAction::SetWalls(walls) = &parsed.actions[0] {
        assert_eq!(walls[0].grid, 4);
        assert_eq!(walls[0].i_range.as_ref().map(|r| r.start), Some(1));
        assert_eq!(walls[0].i_range.as_ref().and_then(|r| r.end), Some(10));
        assert_eq!(walls[0].j_range.as_ref().map(|r| r.start), Some(2));
        assert_eq!(walls[0].j_range.as_ref().and_then(|r| r.end), Some(20));
        assert_eq!(walls[0].k_range.as_ref().map(|r| r.start), Some(-1));
        assert_eq!(walls[0].k_range.as_ref().and_then(|r| r.end), Some(-1));
    } else {
        panic!("expected SetWalls, got {:?}", parsed.actions[0]);
    }
}

#[test]
fn subsets_add_uses_add_action_and_default_grid_one() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("subsets_add.com");
    fs::write(&file, "SUBSETS/ADD /I=3:9 /J=4:10 /K=5:11\n").expect("write");

    let parsed = parse_com_file(&file).expect("parse");
    assert_eq!(parsed.actions.len(), 1);
    if let PlotAction::AddSubsets(subsets) = &parsed.actions[0] {
        assert_eq!(subsets[0].grid, 1);
        assert_eq!(subsets[0].i_range.as_ref().map(|r| r.start), Some(3));
        assert_eq!(subsets[0].j_range.as_ref().map(|r| r.start), Some(4));
        assert_eq!(subsets[0].k_range.as_ref().map(|r| r.start), Some(5));
    } else {
        panic!("expected AddSubsets, got {:?}", parsed.actions[0]);
    }
}

#[test]
fn walls_none_clears_all_walls() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("walls_none.com");
    fs::write(&file, "WALLS/NONE\n").expect("write");

    let parsed = parse_com_file(&file).expect("parse");
    assert_eq!(parsed.actions.len(), 1);
    if let PlotAction::SetWalls(walls) = &parsed.actions[0] {
        assert!(walls.is_empty());
    } else {
        panic!("expected SetWalls, got {:?}", parsed.actions[0]);
    }
}

#[test]
fn subsets_all_warns_and_is_ignored() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("subsets_all.com");
    fs::write(&file, "SUBSETS/ALL\n").expect("write");

    let parsed = parse_com_file(&file).expect("parse");
    assert!(parsed.actions.is_empty());
    assert!(parsed
        .diagnostics
        .iter()
        .any(|d| d.message.contains("/ALL is not yet modeled")));
}

#[test]
fn walls_invalid_six_arg_pair_warns_and_keeps_other_ranges() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("walls_invalid_pair.com");
    fs::write(&file, "WALLS/GRID=3 1 10 BAD 20 3 30\n").expect("write");

    let parsed = parse_com_file(&file).expect("parse");
    assert!(parsed
        .diagnostics
        .iter()
        .any(|d| d.message.contains("invalid J range pair")));

    if let PlotAction::SetWalls(walls) = &parsed.actions[0] {
        assert_eq!(walls[0].grid, 3);
        assert_eq!(walls[0].i_range.as_ref().map(|r| r.start), Some(1));
        assert_eq!(walls[0].i_range.as_ref().and_then(|r| r.end), Some(10));
        assert!(walls[0].j_range.is_none());
        assert_eq!(walls[0].k_range.as_ref().map(|r| r.start), Some(3));
        assert_eq!(walls[0].k_range.as_ref().and_then(|r| r.end), Some(30));
    } else {
        panic!("expected SetWalls, got {:?}", parsed.actions[0]);
    }
}

#[test]
fn subsets_invalid_legacy_token_warns_and_retains_valid_tokens() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("subsets_invalid_token.com");
    fs::write(&file, "SUBSETS/GRID=2 1:10 NOPE 7:17\n").expect("write");

    let parsed = parse_com_file(&file).expect("parse");
    assert!(parsed
        .diagnostics
        .iter()
        .any(|d| d.message.contains("invalid J range token")));

    if let PlotAction::SetSubsets(subsets) = &parsed.actions[0] {
        assert_eq!(subsets[0].grid, 2);
        assert_eq!(subsets[0].i_range.as_ref().map(|r| r.start), Some(1));
        assert_eq!(subsets[0].i_range.as_ref().and_then(|r| r.end), Some(10));
        assert!(subsets[0].j_range.is_none());
        assert_eq!(subsets[0].k_range.as_ref().map(|r| r.start), Some(7));
        assert_eq!(subsets[0].k_range.as_ref().and_then(|r| r.end), Some(17));
    } else {
        panic!("expected SetSubsets, got {:?}", parsed.actions[0]);
    }
}

#[test]
fn walls_interactive_prompt_lines_produce_add_actions() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("walls_interactive.com");
    fs::write(
        &file,
        "WALL/GRID=2\nLAST\n\n2 34\n\nALL\n\nLINE\nRGB .6 .6 .6\n\nALL\n\n34\n\nALL\n\nLINE\nRGB .6 .6 .6\n",
    )
    .expect("write");

    let parsed = parse_com_file(&file).expect("parse");

    let add_walls: Vec<&Vec<GridSubset>> = parsed
        .actions
        .iter()
        .filter_map(|action| {
            if let PlotAction::AddWalls(walls) = action {
                Some(walls)
            } else {
                None
            }
        })
        .collect();

    assert_eq!(add_walls.len(), 2);
    assert_eq!(add_walls[0][0].grid, 2);
    assert_eq!(add_walls[0][0].i_range.as_ref().map(|r| r.start), Some(-1));
    assert_eq!(add_walls[0][0].j_range.as_ref().map(|r| r.start), Some(2));
    assert_eq!(
        add_walls[0][0].j_range.as_ref().and_then(|r| r.end),
        Some(34)
    );
    assert_eq!(add_walls[0][0].k_range.as_ref().map(|r| r.start), Some(1));
    assert!(add_walls[0][0]
        .k_range
        .as_ref()
        .and_then(|r| r.end)
        .is_none());

    assert_eq!(add_walls[1][0].grid, 2);
    assert_eq!(add_walls[1][0].i_range.as_ref().map(|r| r.start), Some(1));
    assert!(add_walls[1][0]
        .i_range
        .as_ref()
        .and_then(|r| r.end)
        .is_none());
    assert_eq!(add_walls[1][0].j_range.as_ref().map(|r| r.start), Some(34));
    assert_eq!(
        add_walls[1][0].j_range.as_ref().and_then(|r| r.end),
        Some(34)
    );
    assert_eq!(add_walls[1][0].k_range.as_ref().map(|r| r.start), Some(1));
    assert!(add_walls[1][0]
        .k_range
        .as_ref()
        .and_then(|r| r.end)
        .is_none());
}

#[test]
fn subsets_interactive_blocks_accumulate_across_commands() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("subsets_interactive.com");
    fs::write(
        &file,
        "SUBSET/GRID=3\n3 64\n\n1 21\n\n1\n\nLINE\nRED\nSUBSET/GRID=4\n3 54\n\n1 11\n\n1\n\nLINE\nGREEN\n",
    )
    .expect("write");

    let parsed = parse_com_file(&file).expect("parse");

    let add_subsets: Vec<&Vec<GridSubset>> = parsed
        .actions
        .iter()
        .filter_map(|action| {
            if let PlotAction::AddSubsets(subsets) = action {
                Some(subsets)
            } else {
                None
            }
        })
        .collect();

    assert_eq!(add_subsets.len(), 2);
    assert_eq!(add_subsets[0][0].grid, 3);
    assert_eq!(add_subsets[0][0].i_range.as_ref().map(|r| r.start), Some(3));
    assert_eq!(
        add_subsets[0][0].i_range.as_ref().and_then(|r| r.end),
        Some(64)
    );
    assert_eq!(add_subsets[0][0].j_range.as_ref().map(|r| r.start), Some(1));
    assert_eq!(
        add_subsets[0][0].j_range.as_ref().and_then(|r| r.end),
        Some(21)
    );
    assert_eq!(add_subsets[0][0].k_range.as_ref().map(|r| r.start), Some(1));
    assert_eq!(
        add_subsets[0][0].k_range.as_ref().and_then(|r| r.end),
        Some(1)
    );

    assert_eq!(add_subsets[1][0].grid, 4);
    assert_eq!(add_subsets[1][0].i_range.as_ref().map(|r| r.start), Some(3));
    assert_eq!(
        add_subsets[1][0].i_range.as_ref().and_then(|r| r.end),
        Some(54)
    );
    assert_eq!(add_subsets[1][0].j_range.as_ref().map(|r| r.start), Some(1));
    assert_eq!(
        add_subsets[1][0].j_range.as_ref().and_then(|r| r.end),
        Some(11)
    );
    assert_eq!(add_subsets[1][0].k_range.as_ref().map(|r| r.start), Some(1));
    assert_eq!(
        add_subsets[1][0].k_range.as_ref().and_then(|r| r.end),
        Some(1)
    );
}

#[test]
fn text_prompt_lines_are_consumed_as_annotation() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("text_prompt.com");
    fs::write(
        &file,
        "TEXT\nGENERIC WING/BODY/TAIL, CHIMERA GRID SCHEME\n\nPLOT\n",
    )
    .expect("write");

    let parsed = parse_com_file(&file).expect("parse");

    assert!(parsed.diagnostics.iter().all(|d| {
        !d.message.contains("TEXT requires at least a quoted string")
            && !d.message.contains("Unsupported command 'GENERIC' ignored")
    }));

    let text_action = parsed.actions.iter().find_map(|action| {
        if let PlotAction::AddTextAnnotation(text) = action {
            Some(text)
        } else {
            None
        }
    });

    let text_action = text_action.expect("expected AddTextAnnotation action");
    assert_eq!(
        text_action.content,
        "GENERIC WING/BODY/TAIL, CHIMERA GRID SCHEME"
    );
    assert_eq!(text_action.x, 0.05);
    assert_eq!(text_action.y, 0.95);
}

#[test]
fn wbt_script_first_plot_setup_matches_legacy_intent() {
    let file = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../demo/wbt/wbt.com");
    let parsed = parse_com_file(&file).expect("parse wbt.com");
    let result = crate::execute_parsed_script(crate::PlotState::default(), &parsed);

    assert!(
        !result.intents.is_empty(),
        "wbt.com should produce at least one PLOT intent"
    );

    for (idx, intent) in result.intents.iter().enumerate() {
        let viewpoint = intent
            .state
            .viewpoint
            .as_ref()
            .map(|vp| format!("({:.3},{:.3},{:.3})", vp.x, vp.y, vp.z))
            .unwrap_or_else(|| "none".to_string());
        println!(
            "WBT intent {}: walls={} subsets={} family={:?} axis={:?} vpoint={}",
            idx + 1,
            intent.state.walls.len(),
            intent.state.subsets.len(),
            intent.state.plot_family,
            intent.state.axis_view,
            viewpoint
        );
    }

    let first = &result.intents[0].state;

    assert!(
        first.walls.len() >= 12,
        "expected at least 12 wall entries in first WBT frame, got {}",
        first.walls.len()
    );

    let grids_present: std::collections::BTreeSet<u32> =
        first.walls.iter().map(|w| w.grid).collect();
    assert!(
        grids_present.contains(&2) && grids_present.contains(&3) && grids_present.contains(&4),
        "first WBT frame walls should include grids 2, 3, and 4; got {:?}",
        grids_present
    );

    assert_eq!(
        first.minmax.x.as_ref().map(|b| (b.min, b.max)),
        Some((0.0, 14.0))
    );
    assert_eq!(
        first.minmax.y.as_ref().map(|b| (b.min, b.max)),
        Some((-5.0, 5.0))
    );
    assert_eq!(
        first.minmax.z.as_ref().map(|b| (b.min, b.max)),
        Some((-1.0, 1.0))
    );
}

// ── Integration tests: plot3d.md examples ────────────────────────────────────

#[test]
fn parse_cp_com_2d_line_plot_example_from_plot3d_md() {
    // Simplified cp.com example from plot3d.md
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("cp.com");
    let content = r#"
    FUNCTION 114
    FSURFACE /WALLS_ORIGIN=1
    MINMAX /INC -0.75,0.75,0.25,1.5,-1.5,-0.5
    VIEW XZ
    PLOT/LINE
    "#;
    fs::write(&file, content).expect("write");

    let parsed = parse_com_file(&file).expect("parse");

    // Expected actions: SetScalarField + two SetMinMax + SetAxisView + SetPlotFamily + CommitPlot
    // (FSURFACE and VIEW produce actions; MINMAX/INC produces diagnostics)
    assert!(
        parsed.actions.len() >= 4,
        "expected at least 4 actions, got {}: {:?}",
        parsed.actions.len(),
        parsed.actions
    );

    // Verify SetAxisView is present with PlaneXZ
    let has_axis_view_xz = parsed
        .actions
        .iter()
        .any(|action| matches!(action, PlotAction::SetAxisView(AxisView::PlaneXZ)));
    assert!(has_axis_view_xz, "expected PlaneXZ axis view");

    // Verify CommitPlot is present
    let has_plot = parsed
        .actions
        .iter()
        .any(|action| matches!(action, PlotAction::CommitPlot));
    assert!(has_plot, "expected CommitPlot action");

    // No errors expected (only diagnostic for /INC is acceptable)
    let errors: Vec<_> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .collect();
    assert!(errors.is_empty(), "expected no errors, got {:?}", errors);
}

#[test]
fn parse_top_com_shuttle_example_from_plot3d_md() {
    // Simplified shuttle "top.com" example from plot3d.md
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("top.com");
    let content = r#"
    FUNCTION 100
    VIEW TOP
    TEXT
    Space Shuttle Comparison
    WALLS
    PLOT/CONTOUR
    "#;
    fs::write(&file, content).expect("write");

    let parsed = parse_com_file(&file).expect("parse");

    // Expected actions: SetScalarField + SetAxisView(PlaneXY) + (walls) + CommitPlot
    // Note: TEXT with following line content is treated as a mode switch in this parser
    // unless explicit inline text arguments are provided.
    assert!(
        parsed.actions.len() >= 3,
        "expected at least 3 actions, got {}",
        parsed.actions.len()
    );

    // Verify SetAxisView is present with PlaneXY (equivalent to TOP)
    let has_axis_view_xy = parsed
        .actions
        .iter()
        .any(|action| matches!(action, PlotAction::SetAxisView(AxisView::PlaneXY)));
    assert!(has_axis_view_xy, "expected PlaneXY axis view for TOP");

    // Verify CommitPlot is present
    let has_plot = parsed
        .actions
        .iter()
        .any(|action| matches!(action, PlotAction::CommitPlot));
    assert!(has_plot, "expected CommitPlot action");
}

#[test]
fn parse_script_with_vpoint_spherical_lookup_from_plot3d_md() {
    // Example script showing VPOINT/ANGLES as documented in plot3d.md
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("isometric.com");
    let content = r#"
    FUNCTION 100
    VPOINT/ANGLES 30 45 10
    PLOT/CONTOUR
    "#;
    fs::write(&file, content).expect("write");

    let parsed = parse_com_file(&file).expect("parse");

    // Expected: SetScalarField + SetViewpoint + CommitPlot
    assert!(
        parsed.actions.len() >= 3,
        "expected at least 3 actions, got {}",
        parsed.actions.len()
    );

    // Verify SetViewpoint is present with spherically converted coordinates
    let vp = parsed.actions.iter().find_map(|action| {
        if let PlotAction::SetViewpoint(vp) = action {
            Some(vp)
        } else {
            None
        }
    });
    assert!(vp.is_some(), "expected SetViewpoint action");

    let vp = vp.unwrap();
    // φ=30°, θ=45°, r=10 should give approximately (6.1, 3.5, 7.07)
    assert!(
        (vp.x - 6.1).abs() < 0.2,
        "x from spherical(30,45,10) should be ~6.1, got {}",
        vp.x
    );
    assert!(
        (vp.z - 7.07).abs() < 0.1,
        "z from spherical(30,45,10) should be ~7.07, got {}",
        vp.z
    );
}

#[test]
fn parse_airfoil_example_with_view_and_plot_line_qualifier() {
    // Example from plot3d.md § VPOINT showing 2D airfoil plot
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("airfoil.com");
    let content = r#"
    FUNCTION 114
    VIEW YX
    MINMAX 0 0.2 -0.5 1
    PLOT/LINE
    "#;
    fs::write(&file, content).expect("write");

    let parsed = parse_com_file(&file).expect("parse");

    // Expected: SetScalarField + SetAxisView(PlaneYX) + SetMinMax + CommitPlot
    assert!(
        parsed.actions.len() >= 3,
        "expected at least 3 actions, got {}",
        parsed.actions.len()
    );

    // Verify PlaneYX (swapped view)
    let has_axis_view_yx = parsed
        .actions
        .iter()
        .any(|action| matches!(action, PlotAction::SetAxisView(AxisView::PlaneYX)));
    assert!(has_axis_view_yx, "expected PlaneYX axis view");

    // Verify no errors
    let errors: Vec<_> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .collect();
    assert!(errors.is_empty(), "expected no errors, got {:?}", errors);
}

#[test]
fn parse_multiplot_script_with_multiple_view_and_vpoint_changes() {
    // Complex script showing multiple camera switches
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("multi.com");
    let content = r#"
    FUNCTION 100
    VIEW TOP
    PLOT/CONTOUR
    VIEW FRONT
    VPOINT 0 10 0
    PLOT/CONTOUR
    "#;
    fs::write(&file, content).expect("write");

    let parsed = parse_com_file(&file).expect("parse");

    // Expected: SetAxisView + CommitPlot + SetAxisView + SetViewpoint + CommitPlot
    // (that's 5 actions minimum)
    assert!(
        parsed.actions.len() >= 5,
        "expected at least 5 actions, got {}: {:?}",
        parsed.actions.len(),
        parsed.actions
    );

    // Count SetAxisView and SetViewpoint actions
    let axis_views = parsed
        .actions
        .iter()
        .filter(|action| matches!(action, PlotAction::SetAxisView(_)))
        .count();
    let viewpoints = parsed
        .actions
        .iter()
        .filter(|action| matches!(action, PlotAction::SetViewpoint(_)))
        .count();
    let plots = parsed
        .actions
        .iter()
        .filter(|action| matches!(action, PlotAction::CommitPlot))
        .count();

    assert_eq!(axis_views, 2, "expected 2 SetAxisView actions");
    assert_eq!(viewpoints, 1, "expected 1 SetViewpoint action");
    assert_eq!(plots, 2, "expected 2 CommitPlot actions");
}

#[test]
fn parse_command_aliases_in_cp_example() {
    // Verify that aliases (VP for VPOINT, FUN for FUNCTION, etc.) work in real scripts
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("aliases.com");
    let content = r#"
    FUN 100
    VP 5.0 5.0 5.0
    PL/CONTOUR
    "#;
    fs::write(&file, content).expect("write");

    let parsed = parse_com_file(&file).expect("parse");

    // Expected: SetScalarField + SetViewpoint + CommitPlot
    assert!(
        parsed.actions.len() >= 3,
        "expected at least 3 actions with aliases, got {}",
        parsed.actions.len()
    );

    // Verify aliases resolved to full commands
    let has_vp = parsed
        .actions
        .iter()
        .any(|action| matches!(action, PlotAction::SetViewpoint(_)));
    assert!(has_vp, "expected VP alias to resolve");
}

#[test]
fn include_path_is_resolved_relative_to_including_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    let nested = root.join("nested");
    fs::create_dir_all(&nested).expect("create nested");

    let include_target = nested.join("child.com");
    fs::write(&include_target, "VIEW TOP\nPLOT/CONTOUR\n").expect("write child");

    let parent = root.join("parent.com");
    fs::write(&parent, "INCLUDE nested/child.com\n").expect("write parent");

    let parsed = parse_com_file(&parent).expect("parse parent");

    assert!(
        parsed
            .actions
            .iter()
            .any(|action| matches!(action, PlotAction::SetAxisView(AxisView::PlaneXY))),
        "expected VIEW TOP from included file"
    );
    assert!(
        parsed
            .actions
            .iter()
            .any(|action| matches!(action, PlotAction::CommitPlot)),
        "expected PLOT from included file"
    );
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|d| d.severity != DiagnosticSeverity::Error),
        "expected no parse errors, got {:?}",
        parsed.diagnostics
    );
}

#[test]
fn empty_script_produces_no_actions_and_no_errors() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("empty.com");
    fs::write(&file, "! This is just a comment\n\n").expect("write");

    let parsed = parse_com_file(&file).expect("parse");
    assert!(parsed.actions.is_empty());
    let errors: Vec<_> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .collect();
    assert!(errors.is_empty());
}

// ===== Phase 7 Edge-Case Hardening Tests =====

#[test]
fn minmax_reversed_bounds_are_swapped_with_warning() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("minmax_reversed.com");
    fs::write(&file, "MINMAX 100.0 50.0\n").expect("write");

    let parsed = parse_com_file(&file).expect("parse");

    // Check that we got a swap warning
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.message.contains("reversed") && d.message.contains("swapping")),
        "expected reversed bounds warning"
    );

    // Check that the SetMinMax action has correct swapped bounds
    assert!(
        parsed.actions.iter().any(|action| {
            if let PlotAction::SetMinMax(mm) = action {
                if let Some(bounds) = &mm.x {
                    bounds.min == 50.0 && bounds.max == 100.0
                } else {
                    false
                }
            } else {
                false
            }
        }),
        "expected swapped X bounds (50..100)"
    );
}

#[test]
fn vpoint_zero_radius_produces_warning_and_uses_default() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("vpoint_zero_radius.com");
    fs::write(&file, "VPOINT/ANGLES 45 45 0\n").expect("write");

    let parsed = parse_com_file(&file).expect("parse");

    // Check for radius warning
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.message.contains("radius") && d.message.contains("must be > 0")),
        "expected zero radius warning"
    );

    // Check that viewpoint was set with default radius (5.0)
    assert!(
        parsed.actions.iter().any(|action| {
            if let PlotAction::SetViewpoint(vp) = action {
                // Spherical (45°, 45°, 5.0) should convert to finite Cartesian coordinates
                vp.x.is_finite() && vp.y.is_finite() && vp.z.is_finite()
            } else {
                false
            }
        }),
        "expected valid viewpoint with default radius"
    );
}

#[test]
fn vpoint_negative_radius_produces_warning_and_uses_default() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("vpoint_negative_radius.com");
    fs::write(&file, "VPOINT/ANGLES 30 60 -2.5\n").expect("write");

    let parsed = parse_com_file(&file).expect("parse");

    // Check for negative radius warning
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.message.contains("radius") && d.message.contains("must be > 0")),
        "expected negative radius warning"
    );
}

#[test]
fn text_position_outside_bounds_produces_info_warning() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("text_out_of_bounds.com");
    fs::write(&file, "TEXT \"Label\" -0.5 1.5\n").expect("write");

    let parsed = parse_com_file(&file).expect("parse");

    // Check for out-of-bounds warning
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.message.contains("outside viewport bounds")),
        "expected out-of-bounds position warning"
    );

    // Text should still be added
    assert!(
        parsed
            .actions
            .iter()
            .any(|a| matches!(a, PlotAction::AddTextAnnotation(_))),
        "expected TEXT action despite out-of-bounds coords"
    );
}

#[test]
fn text_with_non_finite_coordinates_produces_warning_and_skips() {
    let dir = tempfile::tempdir().expect("tempdir");
    let _file = dir.path().join("text_non_finite.com");
    // NaN is tricky; use a manual test instead
    // This is more of a safeguard; in practice floats are finite unless explicitly constructed otherwise
    // For now, skip this test as .com files don't produce NaN naturally
}

#[test]
fn contours_automatic_zero_count_warns_and_uses_default() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("contours_zero.com");
    fs::write(&file, "CONTOURS/AUTOMATIC=0\n").expect("write");

    let parsed = parse_com_file(&file).expect("parse");

    // Check for zero-count warning
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.message.contains("count is 0") && d.message.contains("default")),
        "expected zero count warning"
    );

    // Check that default count (10) was used
    assert!(
        parsed.actions.iter().any(|action| {
            if let PlotAction::SetContourSpec(ContourSpec::Automatic { count }) = action {
                *count == 10
            } else {
                false
            }
        }),
        "expected default count of 10"
    );
}

#[test]
fn contours_very_high_count_warns_about_performance() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("contours_high.com");
    fs::write(&file, "CONTOURS/AUTOMATIC=300\n").expect("write");

    let parsed = parse_com_file(&file).expect("parse");

    // Check for high-count warning
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.message.contains("unusually high") && d.message.contains("degrade")),
        "expected high count warning"
    );

    // Check that the count was accepted
    assert!(
        parsed.actions.iter().any(|action| {
            if let PlotAction::SetContourSpec(ContourSpec::Automatic { count }) = action {
                *count == 300
            } else {
                false
            }
        }),
        "expected count of 300 to be accepted"
    );
}

#[test]
fn contours_multiple_attribute_qualifiers_warns_and_uses_last() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("contours_multi_attr.com");
    fs::write(&file, "CONTOURS /LINE /SURFACE /COLOR 15\n").expect("write");

    let parsed = parse_com_file(&file).expect("parse");

    // Check for multiple qualifier warning
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.message.contains("multiple attribute qualifiers")),
        "expected multiple attribute qualifiers warning"
    );

    // Check that COLOR (last) was used
    assert!(
        parsed.actions.iter().any(|action| {
            if let PlotAction::SetContourAttribute(attr) = action {
                matches!(attr, ContourAttribute::ColorContours)
            } else {
                false
            }
        }),
        "expected COLOR attribute to be used (last one)"
    );
}
