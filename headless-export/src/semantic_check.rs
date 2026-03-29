use clap::Parser;
use image::{ImageReader, RgbaImage};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "overview-export-semantic-check")]
#[command(about = "Compares two PNGs with tolerance-based semantic drift thresholds")]
struct Args {
    /// Label used in output messages.
    #[arg(long)]
    label: String,

    /// Path to generated image from current run.
    #[arg(long)]
    actual: PathBuf,

    /// Path to checked-in reference image.
    #[arg(long)]
    reference: PathBuf,

    /// Maximum allowed mean absolute RGB channel difference (0-255).
    #[arg(long, default_value_t = 0.75)]
    max_mean_error: f64,

    /// Maximum allowed RMS RGB channel difference (0-255).
    #[arg(long, default_value_t = 2.5)]
    max_rms_error: f64,

    /// Maximum allowed ratio of pixels above changed-threshold.
    #[arg(long, default_value_t = 0.005)]
    max_changed_ratio: f64,

    /// Per-channel threshold used to decide if a pixel is changed.
    #[arg(long, default_value_t = 8)]
    changed_threshold: u8,
}

#[derive(Debug)]
struct Metrics {
    mean_error: f64,
    rms_error: f64,
    max_error: u8,
    changed_ratio: f64,
    width: u32,
    height: u32,
}

fn load_rgba(path: &PathBuf) -> Result<RgbaImage, String> {
    let reader = ImageReader::open(path)
        .map_err(|e| format!("failed to open {}: {e}", path.display()))?;
    let decoded = reader
        .decode()
        .map_err(|e| format!("failed to decode {}: {e}", path.display()))?;
    Ok(decoded.to_rgba8())
}

fn compute_metrics(actual: &RgbaImage, reference: &RgbaImage, changed_threshold: u8) -> Metrics {
    let width = actual.width();
    let height = actual.height();
    let total_pixels = (width as u64) * (height as u64);

    let mut sum_abs: u64 = 0;
    let mut sum_sq: u128 = 0;
    let mut max_error: u8 = 0;
    let mut changed_pixels: u64 = 0;

    for (a, b) in actual.pixels().zip(reference.pixels()) {
        let mut pixel_changed = false;

        // Compare RGB only; alpha differences are ignored for renderer drift checks.
        for idx in 0..3 {
            let da = a[idx] as i16;
            let db = b[idx] as i16;
            let diff = (da - db).unsigned_abs() as u8;

            sum_abs += diff as u64;
            sum_sq += (diff as u128) * (diff as u128);
            if diff > max_error {
                max_error = diff;
            }
            if diff > changed_threshold {
                pixel_changed = true;
            }
        }

        if pixel_changed {
            changed_pixels += 1;
        }
    }

    let total_channels = (total_pixels * 3) as f64;
    let mean_error = (sum_abs as f64) / total_channels;
    let rms_error = ((sum_sq as f64) / total_channels).sqrt();
    let changed_ratio = (changed_pixels as f64) / (total_pixels as f64);

    Metrics {
        mean_error,
        rms_error,
        max_error,
        changed_ratio,
        width,
        height,
    }
}

fn main() {
    let args = Args::parse();

    let actual = match load_rgba(&args.actual) {
        Ok(img) => img,
        Err(err) => {
            eprintln!("FAIL: {} semantic check setup error: {}", args.label, err);
            std::process::exit(1);
        }
    };

    let reference = match load_rgba(&args.reference) {
        Ok(img) => img,
        Err(err) => {
            eprintln!("FAIL: {} semantic check setup error: {}", args.label, err);
            std::process::exit(1);
        }
    };

    if actual.dimensions() != reference.dimensions() {
        eprintln!(
            "FAIL: {} dimension mismatch (actual={}x{}, reference={}x{})",
            args.label,
            actual.width(),
            actual.height(),
            reference.width(),
            reference.height()
        );
        std::process::exit(1);
    }

    let metrics = compute_metrics(&actual, &reference, args.changed_threshold);

    println!(
        "SEMANTIC: {} metrics mean_abs={:.4} rms={:.4} max={} changed_ratio={:.6} dims={}x{}",
        args.label,
        metrics.mean_error,
        metrics.rms_error,
        metrics.max_error,
        metrics.changed_ratio,
        metrics.width,
        metrics.height
    );

    let mut failures = Vec::new();
    if metrics.mean_error > args.max_mean_error {
        failures.push(format!(
            "mean_abs {:.4} > max_mean_error {:.4}",
            metrics.mean_error, args.max_mean_error
        ));
    }
    if metrics.rms_error > args.max_rms_error {
        failures.push(format!(
            "rms {:.4} > max_rms_error {:.4}",
            metrics.rms_error, args.max_rms_error
        ));
    }
    if metrics.changed_ratio > args.max_changed_ratio {
        failures.push(format!(
            "changed_ratio {:.6} > max_changed_ratio {:.6}",
            metrics.changed_ratio, args.max_changed_ratio
        ));
    }

    if failures.is_empty() {
        println!("PASS: {} semantic drift within thresholds", args.label);
        return;
    }

    eprintln!("FAIL: {} semantic drift exceeded thresholds", args.label);
    for failure in failures {
        eprintln!("- {}", failure);
    }
    std::process::exit(1);
}
