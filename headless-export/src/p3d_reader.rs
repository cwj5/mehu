/// Adapter over the shared PLOT3D reader stack from `src-tauri`.
///
/// This intentionally reuses the same byte-order / precision detection used
/// by the GUI path to avoid divergence in file-format support.
use std::io;
use std::path::Path;

/// Conservative variable arrays extracted for a single grid.
pub struct QData {
    pub rho: Vec<f32>,
    pub rhou: Vec<f32>,
    pub rhov: Vec<f32>,
    pub rhow: Vec<f32>,
    pub rhoe: Vec<f32>,
    pub gamma: Option<Vec<f32>>,
}

/// Read first grid from a PLOT3D grid file using shared auto-detection.
///
/// Returns `(ni, nj, nk, x_coords, y_coords, z_coords)`.
pub fn read_grid(path: &Path) -> io::Result<(u32, u32, u32, Vec<f32>, Vec<f32>, Vec<f32>)> {
    let grids = match crate::plot3d::read_plot3d_grid(path) {
        Ok(v) => v,
        Err(binary_err) => crate::plot3d::read_plot3d_grid_ascii(path).map_err(|ascii_err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to parse grid as binary ({binary_err}) or ASCII ({ascii_err})"),
            )
        })?,
    };

    let grid = grids.into_iter().next().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "Grid file contained no grids")
    })?;

    Ok((
        grid.dimensions.i,
        grid.dimensions.j,
        grid.dimensions.k,
        grid.x_coords,
        grid.y_coords,
        grid.z_coords,
    ))
}

/// Read first grid from a PLOT3D solution file using shared auto-detection.
///
/// `total` must equal `ni * nj * nk` for the selected grid.
pub fn read_q(path: &Path, total: usize) -> io::Result<QData> {
    let solutions = match crate::plot3d::read_plot3d_solution(path) {
        Ok(v) => v,
        Err(binary_err) => {
            crate::plot3d::read_plot3d_solution_ascii(path).map_err(|ascii_err| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to parse Q as binary ({binary_err}) or ASCII ({ascii_err})"),
                )
            })?
        }
    };

    let sol = solutions.into_iter().next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Q file contained no solution grids",
        )
    })?;

    let got = sol.rho.len();
    if got != total {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Q/grid dimension mismatch: expected {total} points, got {got} in first solution grid"
            ),
        ));
    }

    Ok(QData {
        rho: sol.rho,
        rhou: sol.rhou,
        rhov: sol.rhov,
        rhow: sol.rhow,
        rhoe: sol.rhoe,
        gamma: sol.gamma,
    })
}

/// Compute scalar field values using shared solution equations.
///
/// Returns `(values, field_min, field_max)`.
pub fn compute_scalar(q: &QData, field: &crate::plot_state::ScalarField) -> (Vec<f32>, f32, f32) {
    let total = q.rho.len();
    let solution = crate::plot3d::Plot3DSolution {
        grid_index: 0,
        dimensions: crate::plot3d::GridDimensions {
            i: total as u32,
            j: 1,
            k: 1,
        },
        rho: q.rho.clone(),
        rhou: q.rhou.clone(),
        rhov: q.rhov.clone(),
        rhow: q.rhow.clone(),
        rhoe: q.rhoe.clone(),
        gamma: q.gamma.clone(),
        metadata: None,
    };

    let values = crate::solution::compute_scalar_field(&solution, field.clone());
    let (fmin, fmax) = finite_range(&values);
    (values, fmin, fmax)
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
        (0.0, 1.0)
    } else {
        (mn, mx)
    }
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
            gamma: None,
        };
        let (vals, mn, mx) = compute_scalar(&q, &crate::plot_state::ScalarField::Density);
        assert_eq!(vals, vec![1.0, 2.0, 3.0]);
        assert_eq!(mn, 1.0);
        assert_eq!(mx, 3.0);
    }

    #[test]
    fn compute_scalar_pressure_uses_gamma_if_present() {
        let q = QData {
            rho: vec![1.0],
            rhou: vec![0.0],
            rhov: vec![0.0],
            rhow: vec![0.0],
            rhoe: vec![2.0],
            gamma: Some(vec![1.5]),
        };
        let (vals, _, _) = compute_scalar(&q, &crate::plot_state::ScalarField::Pressure);
        // p = (gamma-1) * rhoe = 0.5 * 2.0 = 1.0
        assert!((vals[0] - 1.0).abs() < 1e-5, "pressure={}", vals[0]);
    }
}
