use crate::function_mapping::map_legacy_function_number;
use crate::plot_state::{
    cap, spherical_to_cartesian, AxisBounds, AxisView, ContourAttribute, ContourEntry, ContourSpec,
    DatasetRef, Diagnostic, DiagnosticSeverity, FsurfaceSpec, GridSubset, IndexRange,
    MinMaxOverride, PlotAction, PlotFamily, PlotText, PlotUpAxis, ScalarField, ViewPoint,
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
            "VIEW requires an axis or plane token (e.g. X, -Z, XY, TOP)",
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
    let is_spherical = args.iter().any(|arg| {
        if let Some((name, _)) = parse_qualifier(arg) {
            name.to_uppercase() == "ANGLES"
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
        spherical_to_cartesian(numeric_args[0], numeric_args[1], numeric_args[2])
    } else {
        // Cartesian coordinates
        (numeric_args[0], numeric_args[1], numeric_args[2])
    };

    out.actions
        .push(PlotAction::SetViewpoint(ViewPoint { x, y, z }));
}

fn parse_minmax(args: &[String], file: &Path, line: u32, out: &mut ParsedScript) {
    // Collect axis-selection qualifiers and numeric values separately.
    // Known non-state qualifiers like /INCREMENT, /XSCALE are silently accepted.
    let mut active_axes: Vec<&str> = Vec::new();
    let mut numeric_args: Vec<f64> = Vec::new();

    for arg in args {
        if let Some((name, _)) = parse_qualifier(arg) {
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
        mm.x = Some(AxisBounds {
            min: numeric_args[0],
            max: numeric_args[1],
        });
        if numeric_args.len() >= 4 {
            mm.y = Some(AxisBounds {
                min: numeric_args[2],
                max: numeric_args[3],
            });
        }
        if numeric_args.len() >= 6 {
            mm.z = Some(AxisBounds {
                min: numeric_args[4],
                max: numeric_args[5],
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
    let attr = if qualifier_values.contains_key("LINE") {
        Some(ContourAttribute::Line)
    } else if qualifier_values.contains_key("SURFACE") {
        Some(ContourAttribute::Surface)
    } else if qualifier_values.contains_key("GRID") {
        Some(ContourAttribute::Grid)
    } else if qualifier_values.contains_key("COLOR") {
        Some(ContourAttribute::ColorContours)
    } else if qualifier_values.contains_key("DOTS") {
        Some(ContourAttribute::Dots)
    } else {
        None
    };
    if let Some(attribute) = attr {
        out.actions.push(PlotAction::SetContourAttribute(attribute));
    }

    // Warn about truly unknown qualifiers for all contour modes.
    for qualifier in qualifier_values.keys() {
        if !matches!(
            qualifier.as_str(),
            "AUTOMATIC"
                | "INCREMENT"
                | "MANUAL"
                | "RANGE"
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

    out.actions
        .push(PlotAction::SetContourSpec(ContourSpec::Automatic { count }));
}

fn parse_plot(args: &[String], file: &Path, line: u32, out: &mut ParsedScript) {
    for arg in args {
        if let Some((name, value)) = parse_qualifier(arg) {
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
                // /2D and /3D are accepted without effect on shared state.
                "2D" | "3D" => {}
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

    out.actions.push(PlotAction::AddTextAnnotation(PlotText {
        content: text,
        x,
        y,
    }));
}

fn parse_show(_file: &Path, _line: u32, out: &mut ParsedScript) {
    out.actions.push(PlotAction::ShowStatus);
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
            format!("{} requires a grid index or /GRID=n qualifier", capability),
        ));
        return;
    }

    let mut grid_from_qualifier: Option<u32> = None;
    let mut add_mode = false;
    let mut positional: Vec<String> = Vec::new();

    for arg in args {
        if let Some((name, value)) = parse_qualifier(arg) {
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

    let grid = if let Some(v) = grid_from_qualifier {
        v
    } else {
        match positional.first().and_then(|s| s.parse::<u32>().ok()) {
            Some(v) if v > 0 => v,
            _ => {
                out.diagnostics.push(diagnostic(
                    capability,
                    DiagnosticSeverity::Warning,
                    Some(file.to_string_lossy().to_string()),
                    Some(line),
                    Some(1),
                    format!(
                        "{} requires a 1-based grid index (positional or /GRID)",
                        capability
                    ),
                ));
                return;
            }
        }
    };

    let mut subset = GridSubset {
        grid,
        gui_managed: false,
        i_range: None,
        j_range: None,
        k_range: None,
    };

    // Range arguments are positional only and follow the optional positional grid.
    // Preferred form: i_start i_end j_start j_end k_start k_end
    // Legacy fallback: i_range j_range k_range (e.g. 1:10 2:20 3:30 or (1,10) ...)
    let range_start = if grid_from_qualifier.is_some() { 0 } else { 1 };
    let range_args = positional.get(range_start..).unwrap_or(&[]);

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
        }
        if let (Some(j_start), Some(j_end)) = (parse_i32_token(2), parse_i32_token(3)) {
            subset.j_range = Some(IndexRange {
                start: j_start,
                end: Some(j_end),
            });
        }
        if let (Some(k_start), Some(k_end)) = (parse_i32_token(4), parse_i32_token(5)) {
            subset.k_range = Some(IndexRange {
                start: k_start,
                end: Some(k_end),
            });
        }
    } else {
        if let Some(range) = range_args.first().and_then(|s| parse_index_range(s)) {
            subset.i_range = Some(range);
        }
        if let Some(range) = range_args.get(1).and_then(|s| parse_index_range(s)) {
            subset.j_range = Some(range);
        }
        if let Some(range) = range_args.get(2).and_then(|s| parse_index_range(s)) {
            subset.k_range = Some(range);
        }
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
        if let Some((name, value)) = parse_qualifier(arg) {
            match name.as_str() {
                "XYZ" | "GRID" => grid_path = value,
                "Q" | "SOLUTION" => solution_path = value,
                // Known qualifiers that don't affect the dataset reference.
                "1D" | "2D" | "3D" | "FORMATTED" | "UNFORMATTED" | "BINARY" | "PLANES"
                | "WHOLE" | "CHECK" | "NOCHECK" | "BLANK" | "NOBLANK" | "MGRID" | "MDATASET"
                | "FUNCTION" => {}
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
            PlotAction::SetPlotFamily(PlotFamily::FunctionSurface)
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

        assert!(parsed.diagnostics.iter().any(|d| d
            .message
            .contains("Unknown MINMAX qualifier '/INC' ignored")));
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
        fs::write(&file, "FUN 100\nVP 1.0 2.0 3.0\nMM -1.0 1.0\nPL/SURFACE\n").expect("write");

        let parsed = parse_com_file(&file).expect("parse");
        // SetScalarField(Density) + SetViewpoint + SetMinMax + SetPlotFamily(FunctionSurface) + CommitPlot
        assert_eq!(parsed.actions.len(), 5);
        assert_eq!(
            parsed.actions[0],
            PlotAction::SetScalarField(ScalarField::Density)
        );
        assert_eq!(
            parsed.actions[1],
            PlotAction::SetViewpoint(ViewPoint {
                x: 1.0,
                y: 2.0,
                z: 3.0
            })
        );
        assert!(matches!(parsed.actions[2], PlotAction::SetMinMax(_)));
        assert_eq!(
            parsed.actions[3],
            PlotAction::SetPlotFamily(PlotFamily::FunctionSurface)
        );
        assert_eq!(parsed.actions[4], PlotAction::CommitPlot);
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
WALLS 1
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
