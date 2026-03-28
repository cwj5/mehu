use clap::Parser;
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

use plot_state::{ContourAttribute, ContourSpec, PlotFamily, PlotState};
use script_executor::{RenderIntent, SolutionSnapshot};

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
        if !show_fringe_points && iblank_data[idx] != 1 {
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
    cmd: PathBuf,

    /// Output PNG path. Multi-PLOT scripts are suffixed _001, _002, ...
    #[arg(long)]
    out: PathBuf,

    /// Output image width in pixels
    #[arg(long, default_value_t = 1280)]
    width: u32,

    /// Output image height in pixels
    #[arg(long, default_value_t = 720)]
    height: u32,
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
    if !args.cmd.exists() {
        return Err(format!("Command file not found: {}", args.cmd.display()));
    }

    let parsed = com_parser::parse_com_file(&args.cmd)?;
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
    let cmd_dir = args.cmd.parent().unwrap_or(Path::new("."));
    for intent in &mut result.intents {
        intent.snapshot = try_load_snapshot(cmd_dir, &intent.state);
    }

    let output_paths = derive_output_paths(&args.out, result.intents.len());

    for (intent, out_path) in result.intents.iter().zip(output_paths.iter()) {
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "Failed to create output directory {}: {e}",
                    parent.display()
                )
            })?;
        }

        let image = render_intent_image(intent, args.width, args.height);
        image
            .save_with_format(out_path, ImageFormat::Png)
            .map_err(|e| format!("Failed to write PNG {}: {e}", out_path.display()))?;

        println!("wrote {}", out_path.display());
    }

    Ok(())
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
    let mut img = RgbaImage::new(width, height);

    if let Some(ref snapshot) = intent.snapshot {
        // Real data available — use the projection-based renderer.
        let mut render_warnings: Vec<String> = Vec::new();
        renderer::render_snapshot(&mut img, snapshot, &intent.state, &mut render_warnings);
        for w in render_warnings {
            eprintln!("[Warning] renderer: {w}");
        }
    } else {
        // No solution data — fall back to the deterministic placeholder so
        // the smoke-test and unit-test paths still produce PNG output.
        render_placeholder(&mut img, intent);
    }

    img
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

    let grid_path = resolve_path(cmd_dir, grid_path_raw);
    let sol_path = resolve_path(cmd_dir, sol_path_raw);

    if !grid_path.exists() || !sol_path.exists() {
        return None;
    }

    let (ni, nj, nk, x, y, z) = p3d_reader::read_grid(&grid_path)
        .map_err(|e| eprintln!("[Warning] Could not read grid {}: {e}", grid_path.display()))
        .ok()?;

    let total = (ni as usize) * (nj as usize) * (nk as usize);
    let q = p3d_reader::read_q(&sol_path, total)
        .map_err(|e| eprintln!("[Warning] Could not read Q {}: {e}", sol_path.display()))
        .ok()?;

    let (scalar, field_min, field_max) = p3d_reader::compute_scalar(&q, &state.scalar_field);

    Some(SolutionSnapshot {
        ni,
        nj,
        nk,
        x,
        y,
        z,
        scalar,
        field_min,
        field_max,
    })
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
            scalar,
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
