use clap::{Parser, ValueEnum};
use font8x8::UnicodeFonts;
use glob::glob;
use image::{ImageFormat, Rgba, RgbaImage};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

#[cfg(test)]
use image::DynamicImage;
#[cfg(test)]
use std::io::Cursor;

#[path = "../../src-tauri/src/com_parser.rs"]
mod com_parser;
#[path = "../../src-tauri/src/function_mapping.rs"]
mod function_mapping;
mod logger;
#[path = "../../src-tauri/src/plot3d.rs"]
mod plot3d;
#[path = "../../src-tauri/src/plot_state.rs"]
mod plot_state;
#[path = "../../src-tauri/src/script_executor.rs"]
mod script_executor;
#[path = "../../src-tauri/src/solution.rs"]
mod solution;

mod colormap;
mod p3d_reader;
mod renderer;

use plot_state::{ContourAttribute, ContourSpec, PlotFamily, ScalarField};
use script_executor::{RenderIntent, SolutionSnapshot};
// Re-export at crate root so `com_parser` tests referencing `crate::execute_parsed_script`
// and `crate::PlotState` compile when included via `#[path]` into this crate.
pub(crate) use plot_state::PlotState;
pub(crate) use script_executor::execute_parsed_script;

/// IBLANK filter mode shim required by shared `plot3d` module APIs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IblankFilterMode {
    Vertex,
    Cell,
}

fn is_hidden_iblank_point(
    iblank: Option<&Vec<i32>>,
    idx: usize,
    respect_iblank: bool,
    show_fringe_points: bool,
) -> bool {
    if let Some(iblank_data) = iblank {
        if respect_iblank && iblank_data[idx] == 0 {
            return true;
        }
        if !show_fringe_points && iblank_data[idx] < 0 {
            return true;
        }
    }
    false
}

#[derive(Debug, Parser)]
#[command(
    name = "overview-export",
    about = "Headless .com -> PNG exporter with minimal system dependencies"
)]
struct Cli {
    /// Input .com script file
    #[arg(long)]
    cmd: Option<PathBuf>,

    /// Output PNG path. Multi-PLOT scripts are suffixed _001, _002, ...
    #[arg(long)]
    out: Option<PathBuf>,

    /// Batch input directory containing .com scripts
    #[arg(long)]
    input_dir: Option<PathBuf>,

    /// Batch output directory for rendered images
    #[arg(long)]
    output_dir: Option<PathBuf>,

    /// Batch input glob pattern, relative to --input-dir (e.g. "*.com" or "**/*.com")
    #[arg(long, default_value = "*.com")]
    pattern: String,

    /// Batch output filename template. Tokens: {stem}, {index}, optional {plot}
    #[arg(long, default_value = "{stem}.png")]
    output_template: String,

    /// Colormap used for scalar visualization
    #[arg(long, value_enum, default_value_t = CliColormap::Viridis)]
    colormap: CliColormap,

    /// Output image width in pixels
    #[arg(long, default_value_t = 1280)]
    width: u32,

    /// Output image height in pixels
    #[arg(long, default_value_t = 720)]
    height: u32,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliColormap {
    Viridis,
    Turbo,
    Rainbow,
    Hot,
    Grayscale,
}

impl From<CliColormap> for colormap::ColormapName {
    fn from(value: CliColormap) -> Self {
        match value {
            CliColormap::Viridis => colormap::ColormapName::Viridis,
            CliColormap::Turbo => colormap::ColormapName::Turbo,
            CliColormap::Rainbow => colormap::ColormapName::Rainbow,
            CliColormap::Hot => colormap::ColormapName::Hot,
            CliColormap::Grayscale => colormap::ColormapName::Grayscale,
        }
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("overview-export: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Cli::parse();

    if args.width == 0 || args.height == 0 {
        return Err("--width and --height must be greater than zero".to_string());
    }

    colormap::set_active(args.colormap.into());

    let batch_mode = args.input_dir.is_some() || args.output_dir.is_some();
    if batch_mode {
        if args.cmd.is_some() || args.out.is_some() {
            return Err(
                "Batch mode (--input-dir/--output-dir) cannot be combined with --cmd/--out"
                    .to_string(),
            );
        }
        let input_dir = args
            .input_dir
            .as_ref()
            .ok_or_else(|| "Batch mode requires --input-dir".to_string())?;
        let output_dir = args
            .output_dir
            .as_ref()
            .ok_or_else(|| "Batch mode requires --output-dir".to_string())?;
        run_batch(
            input_dir,
            output_dir,
            &args.pattern,
            &args.output_template,
            args.width,
            args.height,
        )
    } else {
        let cmd = args
            .cmd
            .as_ref()
            .ok_or_else(|| "Single-file mode requires --cmd".to_string())?;
        let out = args
            .out
            .as_ref()
            .ok_or_else(|| "Single-file mode requires --out".to_string())?;
        if !cmd.exists() {
            return Err(format!("Command file not found: {}", cmd.display()));
        }
        run_single(cmd, out, args.width, args.height)
    }
}

fn run_single(cmd: &Path, out: &Path, width: u32, height: u32) -> Result<(), String> {
    let parsed = com_parser::parse_com_file(cmd)?;
    let mut result = script_executor::execute_parsed_script(PlotState::default(), &parsed);

    for diag in &result.diagnostics {
        eprintln!(
            "[{:?}] {}: {}",
            diag.severity, diag.capability, diag.message
        );
    }

    if result.intents.is_empty() {
        return Err(
            "No render intents were emitted; script did not commit any PLOT boundary".to_string(),
        );
    }

    // Attempt to resolve SolutionSnapshot for each intent.  If the dataset
    // references are absent or the files cannot be loaded, the intent keeps
    // snapshot=None and the placeholder renderer is used instead.
    let cmd_dir = cmd.parent().unwrap_or(Path::new("."));
    for intent in &mut result.intents {
        intent.snapshot = try_load_snapshot(cmd_dir, &intent.state);
    }

    let output_paths = derive_output_paths(out, result.intents.len());

    for (intent, out_path) in result.intents.iter().zip(output_paths.iter()) {
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "Failed to create output directory {}: {e}",
                    parent.display()
                )
            })?;
        }

        let image = render_intent_image_for_cmd(intent, width, height, Some(cmd_dir));
        image
            .save_with_format(out_path, ImageFormat::Png)
            .map_err(|e| format!("Failed to write PNG {}: {e}", out_path.display()))?;

        println!("wrote {}", out_path.display());
    }

    Ok(())
}

fn run_batch(
    input_dir: &Path,
    output_dir: &Path,
    pattern: &str,
    output_template: &str,
    width: u32,
    height: u32,
) -> Result<(), String> {
    if !input_dir.exists() {
        return Err(format!(
            "Batch input directory not found: {}",
            input_dir.display()
        ));
    }

    let cmd_files = collect_batch_inputs(input_dir, pattern)?;
    if cmd_files.is_empty() {
        return Err(format!(
            "No .com files matched pattern '{}' in {}",
            pattern,
            input_dir.display()
        ));
    }

    for (batch_idx, cmd_file) in cmd_files.iter().enumerate() {
        let parsed = com_parser::parse_com_file(cmd_file)?;
        let mut result = script_executor::execute_parsed_script(PlotState::default(), &parsed);

        for diag in &result.diagnostics {
            eprintln!(
                "[{:?}] {}: {} [{}]",
                diag.severity,
                diag.capability,
                diag.message,
                cmd_file.display()
            );
        }

        if result.intents.is_empty() {
            eprintln!(
                "[Warning] No render intents emitted for {}; skipping",
                cmd_file.display()
            );
            continue;
        }

        let cmd_dir = cmd_file.parent().unwrap_or(Path::new("."));
        for intent in &mut result.intents {
            intent.snapshot = try_load_snapshot(cmd_dir, &intent.state);
        }

        let stem = cmd_file
            .file_stem()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("output");

        let output_paths = derive_batch_output_paths(
            output_dir,
            output_template,
            stem,
            batch_idx + 1,
            result.intents.len(),
        )?;

        for (intent, out_path) in result.intents.iter().zip(output_paths.iter()) {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    format!(
                        "Failed to create output directory {}: {e}",
                        parent.display()
                    )
                })?;
            }

            let image = render_intent_image_for_cmd(intent, width, height, Some(cmd_dir));
            image
                .save_with_format(out_path, ImageFormat::Png)
                .map_err(|e| format!("Failed to write PNG {}: {e}", out_path.display()))?;

            println!("wrote {}", out_path.display());
        }
    }

    Ok(())
}

fn collect_batch_inputs(input_dir: &Path, pattern: &str) -> Result<Vec<PathBuf>, String> {
    let search_glob = input_dir.join(pattern).to_string_lossy().to_string();
    let mut files = Vec::new();
    for entry in glob(&search_glob).map_err(|e| format!("Invalid --pattern '{}': {e}", pattern))? {
        let path = entry.map_err(|e| format!("Glob error for '{}': {e}", search_glob))?;
        if path.is_file() {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn derive_batch_output_paths(
    output_dir: &Path,
    output_template: &str,
    stem: &str,
    batch_index: usize,
    plot_count: usize,
) -> Result<Vec<PathBuf>, String> {
    if Path::new(output_template).is_absolute() {
        return Err("--output-template must be a relative path".to_string());
    }

    let mut base = output_template
        .replace("{stem}", stem)
        .replace("{index}", &format!("{batch_index:03}"));

    let token_check = base.replace("{plot}", "001");

    if token_check.contains('{') || token_check.contains('}') {
        return Err(
            "--output-template contains unknown token; supported tokens are {stem}, {index}, {plot}"
                .to_string(),
        );
    }

    if output_template.contains("{plot}") {
        let mut paths = Vec::with_capacity(plot_count.max(1));
        for plot_idx in 1..=plot_count.max(1) {
            let rendered = output_template
                .replace("{stem}", stem)
                .replace("{index}", &format!("{batch_index:03}"))
                .replace("{plot}", &format!("{plot_idx:03}"));
            let mut path = output_dir.join(rendered);
            ensure_png_extension(&mut path);
            paths.push(path);
        }
        return Ok(paths);
    }

    if base.is_empty() {
        base = format!("{stem}.png");
    }

    let mut base_path = output_dir.join(base);
    ensure_png_extension(&mut base_path);
    Ok(derive_output_paths(&base_path, plot_count))
}

fn ensure_png_extension(path: &mut PathBuf) {
    let has_ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    if !has_ext {
        path.set_extension("png");
    }
}

fn derive_output_paths(base: &Path, count: usize) -> Vec<PathBuf> {
    if count <= 1 {
        return vec![base.to_path_buf()];
    }

    let parent = base.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = base
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("output")
        .to_string();
    let ext = base
        .extension()
        .and_then(|e| e.to_str())
        .filter(|e| !e.is_empty())
        .unwrap_or("png");

    (1..=count)
        .map(|idx| parent.join(format!("{stem}_{idx:03}.{ext}")))
        .collect()
}

fn render_intent_image(intent: &RenderIntent, width: u32, height: u32) -> RgbaImage {
    render_intent_image_for_cmd(intent, width, height, None)
}

fn render_intent_image_for_cmd(
    intent: &RenderIntent,
    width: u32,
    height: u32,
    cmd_dir: Option<&Path>,
) -> RgbaImage {
    let mut img = RgbaImage::new(width, height);
    let mut rendered = false;

    if intent.state.scalar_field == ScalarField::None
        && !intent.state.walls.is_empty()
        && intent.state.rakes.is_none()
        && intent.state.particle_function.is_none()
    {
        if let Some(base_dir) = cmd_dir {
            if let Some(grids) = try_load_multigrid_scene(base_dir, &intent.state) {
                let mut render_warnings: Vec<String> = Vec::new();
                renderer::render_multigrid_walls(
                    &mut img,
                    &grids,
                    &intent.state,
                    true,
                    &mut render_warnings,
                );
                for w in render_warnings {
                    eprintln!("[Warning] renderer: {w}");
                }
                rendered = true;
            }
        }
    }

    // Multi-subset scalar surface: when the script specifies SUBSETS with a
    // non-None scalar field, load all grids + solutions and render each subset
    // patch with the scalar colormap.  Skip this path when overlays (rakes or
    // vectors) are active — those need the single-snapshot path so the overlay
    // drawing code can access velocity data.
    if !rendered
        && !intent.state.subsets.is_empty()
        && intent.state.scalar_field != ScalarField::None
        && intent.state.rakes.is_none()
        && intent.state.vectors.is_none()
    {
        if let Some(base_dir) = cmd_dir {
            if let Some((grids, scalars_per_grid, fmin, fmax)) =
                try_load_all_grids_with_scalars(base_dir, &intent.state)
            {
                let mut render_warnings: Vec<String> = Vec::new();
                renderer::render_multigrid_subsets(
                    &mut img,
                    &grids,
                    &scalars_per_grid,
                    fmin,
                    fmax,
                    &intent.state,
                    &mut render_warnings,
                );
                for w in render_warnings {
                    eprintln!("[Warning] renderer: {w}");
                }
                rendered = true;
            }
        }
    }

    if !rendered {
        if let Some(ref snapshot) = intent.snapshot {
            // Real data available — use the projection-based renderer.
            let mut render_warnings: Vec<String> = Vec::new();
            if !intent.state.walls.is_empty() {
                if let Some(base_dir) = cmd_dir {
                    if let Some(grids) = try_load_multigrid_scene(base_dir, &intent.state) {
                        let multigrid_flow =
                            try_load_multigrid_flow_samples(base_dir, &intent.state, &grids);
                        renderer::render_snapshot_with_multigrid_walls(
                            &mut img,
                            snapshot,
                            &grids,
                            multigrid_flow.as_deref(),
                            &intent.state,
                            &mut render_warnings,
                        );
                    } else {
                        renderer::render_snapshot(
                            &mut img,
                            snapshot,
                            &intent.state,
                            &mut render_warnings,
                        );
                    }
                } else {
                    renderer::render_snapshot(
                        &mut img,
                        snapshot,
                        &intent.state,
                        &mut render_warnings,
                    );
                }
            } else {
                renderer::render_snapshot(&mut img, snapshot, &intent.state, &mut render_warnings);
            }
            for w in render_warnings {
                eprintln!("[Warning] renderer: {w}");
            }
        } else {
            // No solution data — fall back to the deterministic placeholder so
            // the smoke-test and unit-test paths still produce PNG output.
            render_placeholder(&mut img, intent);
        }
    }

    draw_text_annotations(&mut img, &intent.state.text_annotations);

    img
}

fn draw_text_annotations(img: &mut RgbaImage, annotations: &[plot_state::PlotText]) {
    if img.width() == 0 || img.height() == 0 || annotations.is_empty() {
        return;
    }

    // Legacy screenshots use smaller/lighter annotation text than the initial
    // MVP overlay; keep exports conservative unless rendering very large frames.
    let scale = if img.height() >= 1000 { 3i32 } else { 2i32 };
    let text_color = if scale >= 3 {
        Rgba([245, 245, 245, 255])
    } else {
        Rgba([232, 232, 232, 255])
    };
    let shadow_color = Rgba([0, 0, 0, 180]);
    let draw_shadow = scale >= 3;
    let glyph_w = 8i32 * scale;
    let glyph_h = 8i32 * scale;
    let line_gap = scale;

    for text in annotations {
        if text.content.trim().is_empty() {
            continue;
        }

        let x = text.x.clamp(0.0, 1.0);
        let y = text.y.clamp(0.0, 1.0);

        let mut pen_x = (x * (img.width().saturating_sub(1)) as f64).round() as i32;
        let mut pen_y = ((1.0 - y) * (img.height().saturating_sub(1)) as f64).round() as i32;
        let origin_x = pen_x;

        for ch in text.content.chars() {
            if ch == '\n' {
                pen_x = origin_x;
                pen_y += glyph_h + line_gap;
                continue;
            }

            let glyph = font8x8::BASIC_FONTS
                .get(ch)
                .or_else(|| font8x8::BASIC_FONTS.get('?'));

            if let Some(bitmap) = glyph {
                if draw_shadow {
                    draw_glyph_bitmap(img, pen_x + 1, pen_y + 1, scale, bitmap, shadow_color);
                }
                draw_glyph_bitmap(img, pen_x, pen_y, scale, bitmap, text_color);
            }

            pen_x += glyph_w;
        }
    }
}

fn draw_glyph_bitmap(
    img: &mut RgbaImage,
    x0: i32,
    y0: i32,
    scale: i32,
    bitmap: [u8; 8],
    color: Rgba<u8>,
) {
    for (row, bits) in bitmap.iter().enumerate() {
        let row_i32 = row as i32;
        for col in 0..8 {
            if (bits & (1u8 << col)) == 0 {
                continue;
            }

            let x = x0 + (col as i32) * scale;
            let y = y0 + row_i32 * scale;

            for dy in 0..scale {
                for dx in 0..scale {
                    let px = x + dx;
                    let py = y + dy;
                    if px >= 0 && py >= 0 && (px as u32) < img.width() && (py as u32) < img.height()
                    {
                        img.put_pixel(px as u32, py as u32, color);
                    }
                }
            }
        }
    }
}

// ─── Placeholder renderer (used when no solution data is available) ───────────

fn render_placeholder(img: &mut RgbaImage, intent: &RenderIntent) {
    let width = img.width();
    let height = img.height();
    let bg = match intent.state.plot_family {
        PlotFamily::Contour => [14u8, 32u8, 56u8, 255u8],
        PlotFamily::FunctionSurface => [58u8, 28u8, 12u8, 255u8],
    };
    for y in 0..height {
        let t = y as f32 / (height.max(1) as f32);
        let shade = (1.0 - 0.22 * t).clamp(0.0, 1.0);
        let px = Rgba([
            (bg[0] as f32 * shade) as u8,
            (bg[1] as f32 * shade) as u8,
            (bg[2] as f32 * shade) as u8,
            255,
        ]);
        for x in 0..width {
            img.put_pixel(x, y, px);
        }
    }
    draw_frame(img, Rgba([216, 226, 255, 255]));
    draw_signature_band(img, intent);
    let contour_count = contour_level_count(&intent.state.contour_spec).min(64);
    if contour_count > 0 {
        match intent.state.contour_attribute {
            ContourAttribute::Dots => {
                draw_contour_dots(img, contour_count, Rgba([240, 240, 245, 255]))
            }
            attr => draw_contour_lines(img, contour_count, attr),
        }
    }
}

// ─── Snapshot loader ──────────────────────────────────────────────────────────

/// Determine the 0-based grid index to load for a snapshot.
///
/// When rakes or vectors are active, use the first subset's grid (subsets are
/// 1-based in PLOT3D, so we subtract 1).  This ensures particle-trace frames
/// sample velocity from the grid that actually contains flow data, not the
/// background chimera block.  Falls back to 0 (first block) for all other frames.
fn snapshot_grid_index(state: &PlotState) -> usize {
    if let Some(grid_index) = state
        .rakes
        .as_ref()
        .and_then(|rakes| rakes.interactive_payload.as_ref())
        .and_then(|payload| {
            payload.entries.iter().find_map(|entry| {
                entry
                    .grid_lines
                    .iter()
                    .flat_map(|line| line.iter())
                    .find_map(|token| {
                        token
                            .parse::<usize>()
                            .ok()
                            .and_then(|grid| grid.checked_sub(1))
                    })
            })
        })
    {
        return grid_index;
    }

    if state.rakes.is_some() || state.vectors.is_some() {
        state
            .subsets
            .first()
            .map(|s| s.grid.saturating_sub(1) as usize)
            .unwrap_or(0)
    } else {
        0
    }
}

/// Try to load grid and solution files referenced in `state.dataset` and
/// compute a `SolutionSnapshot`.
///
/// `cmd_dir` is the directory of the `.com` file so that relative paths in
/// `READ` commands are resolved correctly.
///
/// Returns `None` if:
/// - no dataset was referenced in the script
/// - the files are not found or cannot be parsed
/// - the grid/Q file dimensions are inconsistent
fn try_load_snapshot(cmd_dir: &Path, state: &PlotState) -> Option<SolutionSnapshot> {
    let grid_path_raw = state.dataset.grid_id.as_deref()?;
    let sol_path_raw = state.dataset.solution_id.as_deref()?;

    let grid_path = resolve_data_path(cmd_dir, grid_path_raw, &["fmt"]);
    let sol_path = resolve_data_path(cmd_dir, sol_path_raw, &["fmt"]);

    if !grid_path.exists() || !sol_path.exists() {
        if !grid_path.exists() {
            eprintln!(
                "[Warning] Could not resolve grid dataset path '{}' relative to {}",
                grid_path_raw,
                cmd_dir.display()
            );
        }
        if !sol_path.exists() {
            eprintln!(
                "[Warning] Could not resolve solution dataset path '{}' relative to {}",
                sol_path_raw,
                cmd_dir.display()
            );
        }
        return None;
    }

    let (ni, nj, nk, x, y, z, iblank) =
        p3d_reader::read_grid_n_with_iblank(&grid_path, snapshot_grid_index(state))
            .map_err(|e| eprintln!("[Warning] Could not read grid {}: {e}", grid_path.display()))
            .ok()?;

    let total = (ni as usize) * (nj as usize) * (nk as usize);
    let q = p3d_reader::read_q_n(&sol_path, snapshot_grid_index(state), total)
        .map_err(|e| eprintln!("[Warning] Could not read Q {}: {e}", sol_path.display()))
        .ok()?;

    let (scalar, field_min, field_max) = p3d_reader::compute_scalar(&q, &state.scalar_field);
    let mut u = Vec::with_capacity(total);
    let mut v = Vec::with_capacity(total);
    let mut w = Vec::with_capacity(total);
    for idx in 0..total {
        let rho = q.rho[idx];
        if rho.abs() <= 1e-12 {
            u.push(0.0);
            v.push(0.0);
            w.push(0.0);
            continue;
        }
        u.push(q.rhou[idx] / rho);
        v.push(q.rhov[idx] / rho);
        w.push(q.rhow[idx] / rho);
    }

    Some(SolutionSnapshot {
        ni,
        nj,
        nk,
        x,
        y,
        z,
        iblank,
        scalar,
        u,
        v,
        w,
        field_min,
        field_max,
    })
}

fn try_load_multigrid_scene(cmd_dir: &Path, state: &PlotState) -> Option<Vec<plot3d::Plot3DGrid>> {
    let grid_path_raw = state.dataset.grid_id.as_deref()?;
    let grid_path = resolve_data_path(cmd_dir, grid_path_raw, &["fmt"]);
    if !grid_path.exists() {
        eprintln!(
            "[Warning] Could not resolve grid dataset path '{}' relative to {}",
            grid_path_raw,
            cmd_dir.display()
        );
        return None;
    }

    match plot3d::read_plot3d_grid(&grid_path) {
        Ok(grids) => Some(grids),
        Err(binary_err) => plot3d::read_plot3d_grid_ascii(&grid_path)
            .map_err(|ascii_err| {
                eprintln!(
                    "[Warning] Could not read multigrid scene {}: binary parse failed ({binary_err}), ASCII parse failed ({ascii_err})",
                    grid_path.display()
                )
            })
            .ok(),
    }
}

fn try_load_multigrid_flow_samples(
    cmd_dir: &Path,
    state: &PlotState,
    grids: &[plot3d::Plot3DGrid],
) -> Option<Vec<renderer::FlowSamplePoint>> {
    let sol_path_raw = state.dataset.solution_id.as_deref()?;
    let sol_path = resolve_data_path(cmd_dir, sol_path_raw, &["fmt"]);
    if !sol_path.exists() {
        eprintln!(
            "[Warning] Could not resolve solution dataset path '{}' relative to {}",
            sol_path_raw,
            cmd_dir.display()
        );
        return None;
    }

    let q_per_grid = p3d_reader::read_all_q_for_grids(&sol_path, grids)
        .map_err(|e| {
            eprintln!(
                "[Warning] Could not read multigrid flow samples from {}: {e}",
                sol_path.display()
            )
        })
        .ok()?;

    let mut samples: Vec<renderer::FlowSamplePoint> = Vec::new();
    for (grid_idx, (grid, q)) in grids.iter().zip(q_per_grid.iter()).enumerate() {
        let n_grid = grid.x_coords.len();
        let n_sol = q.rho.len();
        if n_grid == 0 || n_sol == 0 {
            continue;
        }
        let n = n_grid.min(n_sol);
        for idx in 0..n {
            let rho = q.rho[idx];
            let (u, v, w) = if rho.abs() <= 1e-12 {
                (0.0, 0.0, 0.0)
            } else {
                (q.rhou[idx] / rho, q.rhov[idx] / rho, q.rhow[idx] / rho)
            };
            samples.push(renderer::FlowSamplePoint {
                grid_id: grid_idx + 1,
                x: grid.x_coords[idx],
                y: grid.y_coords[idx],
                z: grid.z_coords[idx],
                u,
                v,
                w,
                iblank: grid.iblank.as_ref().and_then(|vals| vals.get(idx)).copied(),
            });
        }
    }

    if samples.is_empty() {
        None
    } else {
        Some(samples)
    }
}

/// Load all grids and their corresponding solution data, compute scalar values
/// for each grid, and return the geometry, per-grid scalar arrays, and the
/// global (field_min, field_max) range.
///
/// Returns `None` when grid/Q paths cannot be resolved or the files cannot be
/// parsed.
fn try_load_all_grids_with_scalars(
    cmd_dir: &Path,
    state: &PlotState,
) -> Option<(Vec<plot3d::Plot3DGrid>, Vec<Vec<f32>>, f32, f32)> {
    let grid_path_raw = state.dataset.grid_id.as_deref()?;
    let sol_path_raw = state.dataset.solution_id.as_deref()?;

    let grid_path = resolve_data_path(cmd_dir, grid_path_raw, &["fmt"]);
    let sol_path = resolve_data_path(cmd_dir, sol_path_raw, &["fmt"]);

    if !grid_path.exists() || !sol_path.exists() {
        if !grid_path.exists() {
            eprintln!(
                "[Warning] Could not resolve grid path '{}' for multi-subset render",
                grid_path_raw
            );
        }
        if !sol_path.exists() {
            eprintln!(
                "[Warning] Could not resolve solution path '{}' for multi-subset render",
                sol_path_raw
            );
        }
        return None;
    }

    let grids = match plot3d::read_plot3d_grid(&grid_path) {
        Ok(v) => v,
        Err(binary_err) => plot3d::read_plot3d_grid_ascii(&grid_path)
            .map_err(|ascii_err| {
                eprintln!(
                    "[Warning] Could not read grid {}: binary ({binary_err}), ASCII ({ascii_err})",
                    grid_path.display()
                )
            })
            .ok()?,
    };

    let q_per_grid = p3d_reader::read_all_q_for_grids(&sol_path, &grids)
        .map_err(|e| {
            eprintln!(
                "[Warning] Could not read Q file {} for multi-subset render: {e}",
                sol_path.display()
            )
        })
        .ok()?;

    let mut scalars_per_grid: Vec<Vec<f32>> = Vec::with_capacity(grids.len());
    let mut global_min = f32::INFINITY;
    let mut global_max = f32::NEG_INFINITY;

    for q in &q_per_grid {
        let (sc, fmin, fmax) = p3d_reader::compute_scalar(q, &state.scalar_field);
        global_min = global_min.min(fmin);
        global_max = global_max.max(fmax);
        scalars_per_grid.push(sc);
    }

    // Pad with empty vecs for grids that had no solution (shouldn't normally happen).
    while scalars_per_grid.len() < grids.len() {
        scalars_per_grid.push(Vec::new());
    }

    if global_min > global_max {
        global_min = 0.0;
        global_max = 1.0;
    }

    Some((grids, scalars_per_grid, global_min, global_max))
}

/// Resolve a path relative to `base_dir` unless it is already absolute.
fn resolve_path(base_dir: &Path, raw: &str) -> PathBuf {
    let p = PathBuf::from(raw);
    if p.is_absolute() {
        p
    } else {
        base_dir.join(p)
    }
}

fn resolve_data_path(base_dir: &Path, raw: &str, fallback_extensions: &[&str]) -> PathBuf {
    let path = resolve_path(base_dir, raw);
    if path.exists() || path.extension().is_some() {
        return path;
    }

    for extension in fallback_extensions {
        let candidate = path.with_extension(extension);
        if candidate.exists() {
            return candidate;
        }
    }

    path
}

// ─── Placeholder drawing helpers (kept for fallback renderer) ─────────────────

fn draw_frame(img: &mut RgbaImage, color: Rgba<u8>) {
    let width = img.width();
    let height = img.height();
    if width < 2 || height < 2 {
        return;
    }

    for x in 0..width {
        img.put_pixel(x, 0, color);
        img.put_pixel(x, height - 1, color);
    }
    for y in 0..height {
        img.put_pixel(0, y, color);
        img.put_pixel(width - 1, y, color);
    }
}

fn draw_signature_band(img: &mut RgbaImage, intent: &RenderIntent) {
    let width = img.width();
    let height = img.height();
    if width == 0 || height == 0 {
        return;
    }

    let mut hasher = DefaultHasher::new();
    format!("{:?}", intent.state).hash(&mut hasher);
    let seed = hasher.finish();

    let band_h = (height / 18).max(6);
    for y in 1..(band_h.min(height - 1)) {
        for x in 1..(width - 1) {
            let m = ((x as u64 * 1103515245 + seed) >> 16) as u8;
            let c = Rgba([40 + (m % 100), 80 + (m % 120), 140 + (m % 100), 255]);
            img.put_pixel(x, y, c);
        }
    }
}

fn contour_level_count(spec: &ContourSpec) -> usize {
    match spec {
        ContourSpec::None => 0,
        ContourSpec::Automatic { count } => *count as usize,
        ContourSpec::Increment { .. } => 12,
        ContourSpec::Manual { entries } => entries.len(),
    }
}

fn draw_contour_lines(img: &mut RgbaImage, count: usize, attr: ContourAttribute) {
    let width = img.width();
    let height = img.height();
    if width < 4 || height < 4 || count == 0 {
        return;
    }

    let top = (height / 6).max(2);
    let bottom = (height - (height / 10)).max(top + 1);
    let span = bottom - top;

    for i in 0..count {
        let y = top + ((i as u32 + 1) * span / (count as u32 + 1));
        let c = contour_color(attr, i, count);
        for x in 2..(width - 2) {
            img.put_pixel(x, y, c);
            if matches!(
                attr,
                ContourAttribute::Surface | ContourAttribute::ColorContours
            ) {
                let y2 = (y + 1).min(height - 2);
                img.put_pixel(x, y2, Rgba([c[0] / 2, c[1] / 2, c[2] / 2, 255]));
            }
            if matches!(attr, ContourAttribute::Grid) && x % 16 == 0 {
                for yy in y.saturating_sub(3)..=(y + 3).min(height - 2) {
                    img.put_pixel(x, yy, c);
                }
            }
        }
    }
}

fn draw_contour_dots(img: &mut RgbaImage, count: usize, color: Rgba<u8>) {
    let width = img.width();
    let height = img.height();
    if width < 4 || height < 4 || count == 0 {
        return;
    }

    let top = (height / 5).max(2);
    let bottom = (height - (height / 8)).max(top + 1);
    let span = bottom - top;
    let spacing = (width / 24).max(8);

    for i in 0..count {
        let y = top + ((i as u32 + 1) * span / (count as u32 + 1));
        let y0 = y.saturating_sub(1);
        let y1 = (y + 1).min(height - 2);
        let mut x = 6;
        while x < width.saturating_sub(6) {
            img.put_pixel(x, y, color);
            img.put_pixel(x, y0, color);
            img.put_pixel(x, y1, color);
            x += spacing;
        }
    }
}

fn contour_color(attr: ContourAttribute, index: usize, total: usize) -> Rgba<u8> {
    match attr {
        ContourAttribute::Line => Rgba([180, 220, 255, 255]),
        ContourAttribute::Surface => Rgba([250, 196, 120, 255]),
        ContourAttribute::Grid => Rgba([186, 248, 170, 255]),
        ContourAttribute::Dots => Rgba([240, 240, 245, 255]),
        ContourAttribute::ColorContours => {
            let t = if total <= 1 {
                0.5
            } else {
                index as f32 / (total as f32 - 1.0)
            };
            // Compact turbo-like ramp without extra dependencies.
            let r = (34.61 + t * (1172.0 - 10793.0 * t + 33300.0 * t * t - 38394.0 * t * t * t))
                .clamp(0.0, 255.0) as u8;
            let g = (23.31 + t * (557.33 - 1225.33 * t + 1700.63 * t * t - 730.26 * t * t * t))
                .clamp(0.0, 255.0) as u8;
            let b = (27.2 + t * (3211.1 - 15327.0 * t + 27814.0 * t * t - 22569.0 * t * t * t))
                .clamp(0.0, 255.0) as u8;
            Rgba([r, g, b, 255])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plot_state::{ContourEntry, ContourSpec, PlotAction};

    #[test]
    fn derive_output_paths_single_uses_exact_path() {
        let out = PathBuf::from("/tmp/result.png");
        let paths = derive_output_paths(&out, 1);
        assert_eq!(paths, vec![PathBuf::from("/tmp/result.png")]);
    }

    #[test]
    fn derive_output_paths_multi_adds_numeric_suffixes() {
        let out = PathBuf::from("/tmp/result.png");
        let paths = derive_output_paths(&out, 3);
        assert_eq!(paths[0], PathBuf::from("/tmp/result_001.png"));
        assert_eq!(paths[1], PathBuf::from("/tmp/result_002.png"));
        assert_eq!(paths[2], PathBuf::from("/tmp/result_003.png"));
    }

    #[test]
    fn derive_batch_output_paths_auto_suffixes_without_plot_token() {
        let out_dir = Path::new("/tmp/out");
        let paths = derive_batch_output_paths(out_dir, "{stem}.png", "wing", 2, 3)
            .expect("batch path derivation should succeed");
        assert_eq!(paths[0], PathBuf::from("/tmp/out/wing_001.png"));
        assert_eq!(paths[1], PathBuf::from("/tmp/out/wing_002.png"));
        assert_eq!(paths[2], PathBuf::from("/tmp/out/wing_003.png"));
    }

    #[test]
    fn derive_batch_output_paths_respects_plot_token() {
        let out_dir = Path::new("/tmp/out");
        let paths = derive_batch_output_paths(out_dir, "{stem}_p{plot}", "wing", 1, 2)
            .expect("batch path derivation should succeed");
        assert_eq!(paths[0], PathBuf::from("/tmp/out/wing_p001.png"));
        assert_eq!(paths[1], PathBuf::from("/tmp/out/wing_p002.png"));
    }

    #[test]
    fn derive_batch_output_paths_rejects_unknown_token() {
        let out_dir = Path::new("/tmp/out");
        let err = derive_batch_output_paths(out_dir, "{stem}_{unknown}.png", "wing", 1, 1)
            .expect_err("unknown token should fail");
        assert!(err.contains("unknown token"));
    }

    #[test]
    fn deterministic_rendering_for_same_intent() {
        let state = PlotState {
            contour_spec: ContourSpec::Manual {
                entries: vec![
                    ContourEntry {
                        value: 0.1,
                        color: None,
                    },
                    ContourEntry {
                        value: 0.9,
                        color: None,
                    },
                ],
            },
            ..PlotState::default()
        };
        let intent = RenderIntent {
            state,
            snapshot: None,
        };

        let a = render_intent_image(&intent, 240, 140);
        let b = render_intent_image(&intent, 240, 140);
        assert_eq!(png_bytes(&a), png_bytes(&b));
    }

    #[test]
    fn render_changes_when_state_changes() {
        let mut state_a = PlotState::default();
        let mut state_b = PlotState::default();
        let _ = plot_state::apply_action(
            state_a.clone(),
            PlotAction::SetContourSpec(ContourSpec::Automatic { count: 4 }),
        );
        state_a.contour_spec = ContourSpec::Automatic { count: 4 };
        state_b.contour_spec = ContourSpec::Automatic { count: 12 };

        let a = render_intent_image(
            &RenderIntent {
                state: state_a,
                snapshot: None,
            },
            240,
            140,
        );
        let b = render_intent_image(
            &RenderIntent {
                state: state_b,
                snapshot: None,
            },
            240,
            140,
        );
        assert_ne!(png_bytes(&a), png_bytes(&b));
    }

    fn png_bytes(img: &RgbaImage) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut cursor = Cursor::new(&mut bytes);
        let dyn_img = DynamicImage::ImageRgba8(img.clone());
        dyn_img
            .write_to(&mut cursor, ImageFormat::Png)
            .expect("encode PNG");
        bytes
    }

    /// Build a small synthetic snapshot: 8×8×1 gradient field.
    fn synthetic_gradient_snapshot() -> SolutionSnapshot {
        let ni = 8u32;
        let nj = 8u32;
        let nk = 1u32;
        let n = (ni * nj * nk) as usize;
        let mut x = Vec::with_capacity(n);
        let mut y = Vec::with_capacity(n);
        let mut z = Vec::with_capacity(n);
        let mut scalar = Vec::with_capacity(n);
        for j in 0..nj as usize {
            for i in 0..ni as usize {
                x.push(i as f32 / (ni as f32 - 1.0));
                y.push(j as f32 / (nj as f32 - 1.0));
                z.push(0.0);
                scalar.push(i as f32 + j as f32 * ni as f32);
            }
        }
        let field_min = 0.0;
        let field_max = ((ni - 1) + (nj - 1) * ni) as f32;
        SolutionSnapshot {
            ni,
            nj,
            nk,
            x,
            y,
            z,
            iblank: None,
            scalar,
            u: vec![1.0; n],
            v: vec![0.0; n],
            w: vec![0.0; n],
            field_min,
            field_max,
        }
    }

    #[test]
    fn snapshot_renderer_produces_colored_pixels() {
        let snap = synthetic_gradient_snapshot();
        let intent = RenderIntent {
            state: PlotState::default(),
            snapshot: Some(snap),
        };
        let img = render_intent_image(&intent, 160, 120);

        // Inner area should contain more than one distinct color (gradient)
        let sample: Vec<_> = (20..140u32)
            .step_by(4)
            .flat_map(|x| (20..100u32).step_by(4).map(move |y| (x, y)))
            .map(|(x, y)| img.get_pixel(x, y).0)
            .collect();
        let unique_colors: std::collections::HashSet<[u8; 4]> = sample.into_iter().collect();
        assert!(
            unique_colors.len() > 4,
            "expected gradient colors, got only {} distinct colors",
            unique_colors.len()
        );
    }

    #[test]
    fn snapshot_renderer_differs_from_placeholder() {
        let snap = synthetic_gradient_snapshot();
        let intent_with_snap = RenderIntent {
            state: PlotState::default(),
            snapshot: Some(snap),
        };
        let intent_no_snap = RenderIntent {
            state: PlotState::default(),
            snapshot: None,
        };
        let with = render_intent_image(&intent_with_snap, 160, 120);
        let without = render_intent_image(&intent_no_snap, 160, 120);
        assert_ne!(
            with.as_raw(),
            without.as_raw(),
            "snapshot renderer should produce different output than placeholder"
        );
    }

    #[test]
    fn snapshot_renderer_with_contours_differs_from_no_contours() {
        let snap = synthetic_gradient_snapshot();
        let state_no_contours = PlotState {
            contour_spec: ContourSpec::None,
            ..PlotState::default()
        };
        let state_with_contours = PlotState {
            contour_spec: ContourSpec::Automatic { count: 5 },
            ..PlotState::default()
        };
        let no_c_img = render_intent_image(
            &RenderIntent {
                state: state_no_contours,
                snapshot: Some(snap.clone()),
            },
            160,
            120,
        );
        let with_c_img = render_intent_image(
            &RenderIntent {
                state: state_with_contours,
                snapshot: Some(snap),
            },
            160,
            120,
        );
        assert_ne!(
            no_c_img.as_raw(),
            with_c_img.as_raw(),
            "contour lines should change the output"
        );
    }
}
