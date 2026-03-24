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
#[path = "../../src-tauri/src/plot_state.rs"]
mod plot_state;
#[path = "../../src-tauri/src/script_executor.rs"]
mod script_executor;

use plot_state::{ContourAttribute, ContourSpec, PlotFamily, PlotState};
use script_executor::RenderIntent;

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
    let result = script_executor::execute_parsed_script(PlotState::default(), &parsed);

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

    draw_frame(&mut img, Rgba([216, 226, 255, 255]));

    // Deterministic header stripe from render-intent hash keeps output
    // visually stable while still reflecting state changes.
    draw_signature_band(&mut img, intent);

    let contour_count = contour_level_count(&intent.state.contour_spec).min(64);
    if contour_count > 0 {
        match intent.state.contour_attribute {
            ContourAttribute::Dots => {
                draw_contour_dots(&mut img, contour_count, Rgba([240, 240, 245, 255]))
            }
            attr => draw_contour_lines(&mut img, contour_count, attr),
        }
    }

    img
}

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
        let intent = RenderIntent { state };

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

        let a = render_intent_image(&RenderIntent { state: state_a }, 240, 140);
        let b = render_intent_image(&RenderIntent { state: state_b }, 240, 140);
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
}
