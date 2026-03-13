use crate::function_mapping::map_legacy_function_number;
use crate::plot_state::{
    cap, AxisView, ContourEntry, ContourSpec, DatasetRef, Diagnostic, DiagnosticSeverity,
    FsurfaceSpec, GridSubset, IndexRange, PlotAction, PlotMode, PlotText, ScalarField, ViewPoint,
};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct ParsedScript {
    pub actions: Vec<PlotAction>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn parse_com_file(path: &Path) -> Result<ParsedScript, String> {
    let mut visited = HashSet::new();
    parse_file_internal(path, &mut visited)
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

    for (idx, raw_line) in content.lines().enumerate() {
        let line_number = (idx + 1) as u32;
        let stripped = strip_comments(raw_line);
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            continue;
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

        parse_command(
            &command,
            &args_with_inline,
            &canonical,
            line_number,
            &mut out,
        );
    }

    visited.remove(&canonical);
    Ok(out)
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
    if include_path.is_absolute() {
        include_path
    } else {
        current_file
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(include_path)
    }
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
        "SHOW" => parse_show(file, line, out),
        "FSURFACE" => parse_fsurface(args, file, line, out),
        "WALLS" => parse_walls_or_subsets(true, args, file, line, out),
        "SUBSETS" | "SUBSET" => parse_walls_or_subsets(false, args, file, line, out),
        "READ" => parse_read(args, file, line, out),
        unsupported => out.diagnostics.push(diagnostic(
            cap::READ,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            format!("Unsupported command '{}' ignored", unsupported),
        )),
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
            let (mapped, mut diags) = map_legacy_function_number(number);
            for diag in &mut diags {
                diag.file = Some(file.to_string_lossy().to_string());
                diag.line = Some(line);
                diag.column = Some(1);
            }
            out.diagnostics.extend(diags);
            if let Some(field) = mapped {
                out.actions.push(PlotAction::SetScalarField(field));
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
            "VIEW requires an axis token (e.g. X, -Z)",
        ));
        return;
    }

    let axis = args[0].to_uppercase();
    let mode = match axis.as_str() {
        "X" | "+X" => Some(AxisView::PlusX),
        "-X" => Some(AxisView::MinusX),
        "Y" | "+Y" => Some(AxisView::PlusY),
        "-Y" => Some(AxisView::MinusY),
        "Z" | "+Z" => Some(AxisView::PlusZ),
        "-Z" => Some(AxisView::MinusZ),
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
                "Unsupported VIEW argument '{}' (expected X/Y/Z or signed axis)",
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
    if args.len() < 3 {
        out.diagnostics.push(diagnostic(
            cap::VPOINT,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            "VPOINT requires 3 numeric values: VPOINT x y z",
        ));
        return;
    }

    let x = parse_f64(&args[0]);
    let y = parse_f64(&args[1]);
    let z = parse_f64(&args[2]);

    match (x, y, z) {
        (Some(x), Some(y), Some(z)) => {
            out.actions
                .push(PlotAction::SetViewpoint(ViewPoint { x, y, z }))
        }
        _ => out.diagnostics.push(diagnostic(
            cap::VPOINT,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            "VPOINT values must be numeric",
        )),
    }
}

fn parse_minmax(args: &[String], file: &Path, line: u32, out: &mut ParsedScript) {
    if args.len() < 2 {
        out.diagnostics.push(diagnostic(
            cap::MINMAX,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            "MINMAX requires at least min and max values",
        ));
        return;
    }

    let min = parse_f64(&args[0]);
    let max = parse_f64(&args[1]);
    match (min, max) {
        (Some(min), Some(max)) => {
            out.actions
                .push(PlotAction::SetMinMax(crate::plot_state::MinMaxOverride {
                    min: Some(min),
                    max: Some(max),
                }));
            if args.len() > 2 {
                out.diagnostics.push(diagnostic(
                    cap::MINMAX,
                    DiagnosticSeverity::Info,
                    Some(file.to_string_lossy().to_string()),
                    Some(line),
                    Some(1),
                    "MINMAX extra values detected; parser currently uses the first min/max pair",
                ));
            }
        }
        _ => out.diagnostics.push(diagnostic(
            cap::MINMAX,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            "MINMAX values must be numeric",
        )),
    }
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
    let mut manual_values: Vec<f64> = Vec::new();

    for arg in args {
        if let Some((name, value)) = parse_qualifier(arg) {
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
                        manual_values.push(v);
                        v += increment;
                    }
                }
            } else {
                manual_values.extend(tuple_values);
            }
            continue;
        }

        if let Some(number) = parse_f64(arg) {
            manual_values.push(number);
            continue;
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

    if let Some(value) = qualifier_values
        .get("AUTOMATIC")
        .and_then(|v| v.as_ref())
        .and_then(|s| s.parse::<u32>().ok())
    {
        out.actions
            .push(PlotAction::SetContourSpec(ContourSpec::Automatic {
                count: value,
            }));
        return;
    }

    if qualifier_values.contains_key("AUTOMATIC") {
        out.actions
            .push(PlotAction::SetContourSpec(ContourSpec::Automatic {
                count: 10,
            }));
        return;
    }

    if let Some(increment_value) = qualifier_values
        .get("INCREMENT")
        .and_then(|v| v.as_ref())
        .and_then(|s| s.parse::<f64>().ok())
    {
        out.actions
            .push(PlotAction::SetContourSpec(ContourSpec::Increment {
                start: 0.0,
                increment: increment_value,
            }));
        return;
    }

    if qualifier_values.contains_key("MANUAL") || !manual_values.is_empty() {
        let entries = manual_values
            .into_iter()
            .map(|value| ContourEntry { value, color: None })
            .collect::<Vec<_>>();
        out.actions
            .push(PlotAction::SetContourSpec(ContourSpec::Manual { entries }));
        return;
    }

    for qualifier in qualifier_values.keys() {
        if qualifier != "AUTOMATIC" && qualifier != "INCREMENT" && qualifier != "MANUAL" {
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
}

fn parse_plot(args: &[String], file: &Path, line: u32, out: &mut ParsedScript) {
    for arg in args {
        if let Some((name, _)) = parse_qualifier(arg) {
            match name.as_str() {
                "SURFACE" | "CARPET" => out
                    .actions
                    .push(PlotAction::SetPlotMode(PlotMode::Surface3d)),
                "LINE" => out.actions.push(PlotAction::SetPlotMode(PlotMode::Lines)),
                "CONTOUR" => out
                    .actions
                    .push(PlotAction::SetPlotMode(PlotMode::Contours)),
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

    out.actions.push(PlotAction::AddTextAnnotation(PlotText {
        content: text,
        x,
        y,
    }));
}

fn parse_show(file: &Path, line: u32, out: &mut ParsedScript) {
    out.diagnostics.push(diagnostic(
        cap::SHOW,
        DiagnosticSeverity::Info,
        Some(file.to_string_lossy().to_string()),
        Some(line),
        Some(1),
        "SHOW parsed; execution behavior is handled by command executor (TKT-005)",
    ));
}

fn parse_fsurface(args: &[String], file: &Path, line: u32, out: &mut ParsedScript) {
    if args.is_empty() {
        out.diagnostics.push(diagnostic(
            cap::FSURFACE,
            DiagnosticSeverity::Warning,
            Some(file.to_string_lossy().to_string()),
            Some(line),
            Some(1),
            "FSURFACE requires a value or /NONE",
        ));
        return;
    }

    if let Some((name, _)) = parse_qualifier(&args[0]) {
        if name == "NONE" || name == "OFF" {
            out.actions.push(PlotAction::SetFsurface(None));
            return;
        }
    }

    if let Some(value) = parse_f64(&args[0]) {
        let field = if let Some(number) = args.get(1).and_then(|s| s.parse::<u16>().ok()) {
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
            format!("Invalid FSURFACE value '{}'", args[0]),
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
            format!("{} requires at least a grid index", capability),
        ));
        return;
    }

    let grid = match args[0].parse::<u32>() {
        Ok(v) if v > 0 => v,
        _ => {
            out.diagnostics.push(diagnostic(
                capability,
                DiagnosticSeverity::Warning,
                Some(file.to_string_lossy().to_string()),
                Some(line),
                Some(1),
                format!("{} first argument must be a 1-based grid index", capability),
            ));
            return;
        }
    };

    let mut subset = GridSubset {
        grid,
        i_range: None,
        j_range: None,
        k_range: None,
    };

    if let Some(range) = args.get(1).and_then(|s| parse_index_range(s)) {
        subset.i_range = Some(range);
    }
    if let Some(range) = args.get(2).and_then(|s| parse_index_range(s)) {
        subset.j_range = Some(range);
    }
    if let Some(range) = args.get(3).and_then(|s| parse_index_range(s)) {
        subset.k_range = Some(range);
    }

    if walls {
        out.actions.push(PlotAction::SetWalls(vec![subset]));
    } else {
        out.actions.push(PlotAction::SetSubsets(vec![subset]));
    }
}

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

    // Temporary parse representation: store raw path strings in dataset IDs so
    // executor can resolve/cache them in TKT-005.
    let dataset = DatasetRef {
        grid_id: args.first().cloned(),
        solution_id: args.get(1).cloned(),
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
            let start = values[0] as u32;
            let end = values[1] as u32;
            return Some(IndexRange {
                start,
                end: Some(end),
            });
        }
    }

    if let Some((a, b)) = t.split_once(':') {
        let start = a.trim().parse::<u32>().ok()?;
        let end = if b.trim().is_empty() {
            None
        } else {
            Some(b.trim().parse::<u32>().ok()?)
        };
        return Some(IndexRange { start, end });
    }

    if let Ok(single) = t.parse::<u32>() {
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

fn resolve_command_alias(command: &str) -> String {
    let upper = command.to_uppercase();
    let alias = command_aliases();
    alias
        .get(upper.as_str())
        .copied()
        .unwrap_or(upper.as_str())
        .to_string()
}

fn command_aliases() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("R", "READ"),
        ("RD", "READ"),
        ("FUN", "FUNCTION"),
        ("FUNC", "FUNCTION"),
        ("V", "VIEW"),
        ("VP", "VPOINT"),
        ("MM", "MINMAX"),
        ("CON", "CONTOURS"),
        ("CONT", "CONTOURS"),
        ("PL", "PLOT"),
        ("WAL", "WALLS"),
        ("SUB", "SUBSETS"),
        ("FS", "FSURFACE"),
        ("TX", "TEXT"),
        ("SH", "SHOW"),
        ("INC", "INCLUDE"),
        ("INCL", "INCLUDE"),
    ])
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
            '"' => {
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
    fn function_known_unimplemented_warns_and_soft_fails() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("two.com");
        fs::write(&file, "FUNCTION 154\n").expect("write script");

        let parsed = parse_com_file(&file).expect("parse script");
        assert!(parsed.actions.is_empty());
        assert_eq!(parsed.diagnostics.len(), 1);
        assert!(parsed.diagnostics[0]
            .message
            .contains("recognized but not implemented"));
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
            PlotAction::SetPlotMode(PlotMode::Surface3d)
        );
        assert_eq!(parsed.actions[1], PlotAction::CommitPlot);
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
}
