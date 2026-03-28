/// Minimal PLOT3D binary file reader for the headless CLI.
///
/// Supports the most common case: single-precision (f32), little-endian,
/// Fortran unformatted binary, single-grid files.  Returns `Err` for anything
/// it cannot parse so callers can fall back gracefully to the placeholder
/// renderer.
///
/// This is intentionally a standalone module — it does not depend on `rayon`
/// or other heavy deps from `src-tauri/src/plot3d.rs`.

use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::Path;

// ─── Low-level I/O helpers ──────────────────────────────────────────────────

fn read_u32_le(r: &mut impl Read) -> io::Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

fn bytes_to_i32s_le(bytes: &[u8]) -> Vec<i32> {
    bytes
        .chunks_exact(4)
        .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn bytes_to_f32s_le(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Read one Fortran unformatted record (little-endian record markers).
/// Returns the raw payload bytes.
fn read_record(r: &mut impl Read) -> io::Result<Vec<u8>> {
    let len = read_u32_le(r)? as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    let end = read_u32_le(r)? as usize;
    if len != end {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Fortran record marker mismatch: start={len} end={end}"),
        ));
    }
    Ok(buf)
}

// ─── Grid reader ────────────────────────────────────────────────────────────

/// Read a single-precision, little-endian, Fortran-unformatted PLOT3D grid
/// file.  Only first grid of a multi-grid file is read.
///
/// Returns `(ni, nj, nk, x_coords, y_coords, z_coords)`.
pub fn read_grid(path: &Path) -> io::Result<(u32, u32, u32, Vec<f32>, Vec<f32>, Vec<f32>)> {
    let mut r = BufReader::new(File::open(path)?);

    // Record 1: ngrids
    let rec1 = read_record(&mut r)?;
    let ngrids_vals = bytes_to_i32s_le(&rec1);
    if ngrids_vals.is_empty() || ngrids_vals[0] <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid ngrids in grid file",
        ));
    }

    // Record 2: ni, nj, nk for each grid (we read the first grid only)
    let rec2 = read_record(&mut r)?;
    let dims = bytes_to_i32s_le(&rec2);
    if dims.len() < 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Not enough dimension data in grid file",
        ));
    }
    let ni = dims[0] as u32;
    let nj = dims[1] as u32;
    let nk = dims[2] as u32;
    if ni == 0 || nj == 0 || nk == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Zero dimension in grid file: {ni}×{nj}×{nk}"),
        ));
    }

    let total = (ni as usize) * (nj as usize) * (nk as usize);

    // Record 3: coordinate data for grid 0 (X sequential, then Y, then Z)
    let rec3 = read_record(&mut r)?;
    let coords = bytes_to_f32s_le(&rec3);
    if coords.len() < total * 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Grid coordinate record too short: expected ≥{} f32 values, got {}",
                total * 3,
                coords.len()
            ),
        ));
    }

    let x = coords[..total].to_vec();
    let y = coords[total..2 * total].to_vec();
    let z = coords[2 * total..3 * total].to_vec();

    Ok((ni, nj, nk, x, y, z))
}

// ─── Q (solution) reader ────────────────────────────────────────────────────

/// Raw conservative variable arrays from a PLOT3D Q file.
pub struct QData {
    pub rho: Vec<f32>,
    pub rhou: Vec<f32>,
    pub rhov: Vec<f32>,
    pub rhow: Vec<f32>,
    pub rhoe: Vec<f32>,
}

/// Read a single-precision, little-endian, Fortran-unformatted PLOT3D Q file.
/// Only the first grid is read.  `total` must equal ni×nj×nk from the grid.
pub fn read_q(path: &Path, total: usize) -> io::Result<QData> {
    let mut r = BufReader::new(File::open(path)?);

    // Record 1: ngrids — skip
    let _ = read_record(&mut r)?;
    // Record 2: dimensions — skip (we already have them from the grid file)
    let _ = read_record(&mut r)?;
    // Record 3: metadata (refmach, alpha, rey, time, …) — skip
    let _ = read_record(&mut r)?;

    // Record 4: q data (rho, rhou, rhov, rhow, rhoe, sequential in blocks of `total`)
    let rec4 = read_record(&mut r)?;
    let q = bytes_to_f32s_le(&rec4);
    if q.len() < total * 5 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Q record too short: expected ≥{} f32 values, got {}",
                total * 5,
                q.len()
            ),
        ));
    }

    Ok(QData {
        rho: q[..total].to_vec(),
        rhou: q[total..2 * total].to_vec(),
        rhov: q[2 * total..3 * total].to_vec(),
        rhow: q[3 * total..4 * total].to_vec(),
        rhoe: q[4 * total..5 * total].to_vec(),
    })
}

// ─── Scalar field computation ───────────────────────────────────────────────

/// Compute a scalar field from conservative variables.
///
/// Mirrors `src-tauri/src/solution.rs::compute_scalar_field`.  Returns
/// `(values, field_min, field_max)`.  Unknown/unimplemented fields fall back
/// to density so the renderer always produces output.
pub fn compute_scalar(
    q: &QData,
    field: &crate::plot_state::ScalarField,
) -> (Vec<f32>, f32, f32) {
    use crate::plot_state::ScalarField;

    let n = q.rho.len();
    let mut result = Vec::with_capacity(n);

    match field {
        ScalarField::Density => result = q.rho.clone(),

        ScalarField::UVelocity => {
            for i in 0..n {
                result.push(if q.rho[i] > 0.0 {
                    q.rhou[i] / q.rho[i]
                } else {
                    0.0
                });
            }
        }

        ScalarField::VVelocity => {
            for i in 0..n {
                result.push(if q.rho[i] > 0.0 {
                    q.rhov[i] / q.rho[i]
                } else {
                    0.0
                });
            }
        }

        ScalarField::WVelocity => {
            for i in 0..n {
                result.push(if q.rho[i] > 0.0 {
                    q.rhow[i] / q.rho[i]
                } else {
                    0.0
                });
            }
        }

        ScalarField::VelocityMagnitude => {
            for i in 0..n {
                let r = q.rho[i];
                if r > 0.0 {
                    let u = q.rhou[i] / r;
                    let v = q.rhov[i] / r;
                    let w = q.rhow[i] / r;
                    result.push((u * u + v * v + w * w).sqrt());
                } else {
                    result.push(0.0);
                }
            }
        }

        ScalarField::Pressure => {
            const GAMMA: f32 = 1.4;
            for i in 0..n {
                let r = q.rho[i];
                if r > 0.0 {
                    let u = q.rhou[i] / r;
                    let v = q.rhov[i] / r;
                    let w = q.rhow[i] / r;
                    let ke = 0.5 * r * (u * u + v * v + w * w);
                    result.push((GAMMA - 1.0) * (q.rhoe[i] - ke));
                } else {
                    result.push(0.0);
                }
            }
        }

        ScalarField::Energy => result = q.rhoe.clone(),
        ScalarField::MomentumX => result = q.rhou.clone(),
        ScalarField::MomentumY => result = q.rhov.clone(),
        ScalarField::MomentumZ => result = q.rhow.clone(),

        // All other variants fall back to density so the renderer always has
        // something to display.
        _ => result = q.rho.clone(),
    }

    let (fmin, fmax) = finite_range(&result);
    (result, fmin, fmax)
}

fn finite_range(values: &[f32]) -> (f32, f32) {
    let mut mn = f32::INFINITY;
    let mut mx = f32::NEG_INFINITY;
    for &v in values {
        if v.is_finite() {
            mn = mn.min(v);
            mx = mx.max(v);
        }
    }
    if mn > mx {
        // All values were non-finite or slice was empty
        mn = 0.0;
        mx = 1.0;
    }
    (mn, mx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finite_range_normal() {
        let (mn, mx) = finite_range(&[1.0, 3.0, 2.0]);
        assert_eq!(mn, 1.0);
        assert_eq!(mx, 3.0);
    }

    #[test]
    fn finite_range_empty_returns_unit() {
        let (mn, mx) = finite_range(&[]);
        assert_eq!(mn, 0.0);
        assert_eq!(mx, 1.0);
    }

    #[test]
    fn compute_scalar_density_passthrough() {
        let q = QData {
            rho: vec![1.0, 2.0, 3.0],
            rhou: vec![0.0; 3],
            rhov: vec![0.0; 3],
            rhow: vec![0.0; 3],
            rhoe: vec![0.0; 3],
        };
        let (vals, mn, mx) = compute_scalar(&q, &crate::plot_state::ScalarField::Density);
        assert_eq!(vals, vec![1.0, 2.0, 3.0]);
        assert_eq!(mn, 1.0);
        assert_eq!(mx, 3.0);
    }

    #[test]
    fn compute_scalar_pressure_positive() {
        // Simple case: zero velocity, so p = (gamma-1) * rhoe
        let n = 4;
        let rho = vec![1.0f32; n];
        let rho0 = vec![0.0f32; n];
        let rhoe = vec![2.5f32; n]; // p = 0.4 * 2.5 = 1.0
        let q = QData {
            rho: rho.clone(),
            rhou: rho0.clone(),
            rhov: rho0.clone(),
            rhow: rho0.clone(),
            rhoe,
        };
        let (vals, mn, mx) = compute_scalar(&q, &crate::plot_state::ScalarField::Pressure);
        for v in &vals {
            assert!((*v - 1.0).abs() < 1e-5, "pressure={v}");
        }
        assert!((mn - 1.0).abs() < 1e-5);
        assert!((mx - 1.0).abs() < 1e-5);
    }
}
