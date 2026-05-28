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
    read_grid_n(path, 0)
}

/// Read the `grid_index`-th grid (0-based) from a PLOT3D grid file.
///
/// Falls back to grid 0 if the requested index is out of range.
pub fn read_grid_n(
    path: &Path,
    grid_index: usize,
) -> io::Result<(u32, u32, u32, Vec<f32>, Vec<f32>, Vec<f32>)> {
    let (ni, nj, nk, x, y, z, _) = read_grid_n_with_iblank(path, grid_index)?;
    Ok((ni, nj, nk, x, y, z))
}

/// Read the `grid_index`-th grid (0-based) and optional IBLANK array.
pub fn read_grid_n_with_iblank(
    path: &Path,
    grid_index: usize,
) -> io::Result<(u32, u32, u32, Vec<f32>, Vec<f32>, Vec<f32>, Option<Vec<i32>>)> {
    let grids = match crate::plot3d::read_plot3d_grid(path) {
        Ok(v) => v,
        Err(binary_err) => crate::plot3d::read_plot3d_grid_ascii(path).map_err(|ascii_err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to parse grid as binary ({binary_err}) or ASCII ({ascii_err})"),
            )
        })?,
    };

    let len = grids.len();
    let idx = if grid_index < len { grid_index } else { 0 };
    let grid = grids.into_iter().nth(idx).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "Grid file contained no grids")
    })?;

    Ok((
        grid.dimensions.i,
        grid.dimensions.j,
        grid.dimensions.k,
        grid.x_coords,
        grid.y_coords,
        grid.z_coords,
        grid.iblank,
    ))
}

/// Read first grid from a PLOT3D solution file using shared auto-detection.
///
/// `total` must equal `ni * nj * nk` for the selected grid.
pub fn read_q(path: &Path, total: usize) -> io::Result<QData> {
    read_q_n(path, 0, total)
}

/// Read the `grid_index`-th solution (0-based) from a PLOT3D solution file.
///
/// Falls back to solution 0 if the requested index is out of range.
pub fn read_q_n(path: &Path, grid_index: usize, total: usize) -> io::Result<QData> {
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

    let len = solutions.len();
    let idx = if grid_index < len { grid_index } else { 0 };
    let sol = solutions.into_iter().nth(idx).ok_or_else(|| {
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

/// Read ALL grids' solution data, returning one `QData` per grid.
///
/// The `grids` slice is used only for dimension validation; if the Q file has
/// fewer grids than the grid file the extra geometry grids are simply not
/// accompanied by solution data and those entries are omitted from the result.
pub fn read_all_q_for_grids(
    path: &Path,
    grids: &[crate::plot3d::Plot3DGrid],
) -> io::Result<Vec<QData>> {
    let solutions = match crate::plot3d::read_plot3d_solution(path) {
        Ok(v) => v,
        Err(binary_err) => crate::plot3d::read_plot3d_solution_ascii(path).map_err(|ae| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to parse Q as binary ({binary_err}) or ASCII ({ae})"),
            )
        })?,
    };

    let mut result = Vec::new();
    for (idx, sol) in solutions.into_iter().enumerate() {
        let expected = grids
            .get(idx)
            .map(|g| g.dimensions.i as usize * g.dimensions.j as usize * g.dimensions.k as usize)
            .unwrap_or(0);
        let got = sol.rho.len();
        if expected > 0 && got != expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Q/grid dimension mismatch for solution grid {}: expected {expected} points, got {got}",
                    idx + 1
                ),
            ));
        }
        result.push(QData {
            rho: sol.rho,
            rhou: sol.rhou,
            rhov: sol.rhov,
            rhow: sol.rhow,
            rhoe: sol.rhoe,
            gamma: sol.gamma,
        });
    }
    Ok(result)
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

    #[test]
    fn compute_scalar_u_velocity() {
        let q = QData {
            rho: vec![2.0, 4.0],
            rhou: vec![4.0, 12.0],
            rhov: vec![0.0; 2],
            rhow: vec![0.0; 2],
            rhoe: vec![0.0; 2],
            gamma: None,
        };
        let (vals, _, _) = compute_scalar(&q, &crate::plot_state::ScalarField::UVelocity);
        // u[0] = rhou[0] / rho[0] = 4.0 / 2.0 = 2.0
        // u[1] = rhou[1] / rho[1] = 12.0 / 4.0 = 3.0
        assert_eq!(vals.len(), 2);
        assert!((vals[0] - 2.0).abs() < 1e-5, "u[0]={}", vals[0]);
        assert!((vals[1] - 3.0).abs() < 1e-5, "u[1]={}", vals[1]);
    }

    #[test]
    fn compute_scalar_v_velocity() {
        let q = QData {
            rho: vec![1.0, 2.0],
            rhou: vec![0.0; 2],
            rhov: vec![5.0, 8.0],
            rhow: vec![0.0; 2],
            rhoe: vec![0.0; 2],
            gamma: None,
        };
        let (vals, _, _) = compute_scalar(&q, &crate::plot_state::ScalarField::VVelocity);
        // v[0] = rhov[0] / rho[0] = 5.0 / 1.0 = 5.0
        // v[1] = rhov[1] / rho[1] = 8.0 / 2.0 = 4.0
        assert_eq!(vals.len(), 2);
        assert!((vals[0] - 5.0).abs() < 1e-5, "v[0]={}", vals[0]);
        assert!((vals[1] - 4.0).abs() < 1e-5, "v[1]={}", vals[1]);
    }

    #[test]
    fn compute_scalar_w_velocity() {
        let q = QData {
            rho: vec![1.0, 2.0],
            rhou: vec![0.0; 2],
            rhov: vec![0.0; 2],
            rhow: vec![3.0, 10.0],
            rhoe: vec![0.0; 2],
            gamma: None,
        };
        let (vals, _, _) = compute_scalar(&q, &crate::plot_state::ScalarField::WVelocity);
        // w[0] = rhow[0] / rho[0] = 3.0 / 1.0 = 3.0
        // w[1] = rhow[1] / rho[1] = 10.0 / 2.0 = 5.0
        assert_eq!(vals.len(), 2);
        assert!((vals[0] - 3.0).abs() < 1e-5, "w[0]={}", vals[0]);
        assert!((vals[1] - 5.0).abs() < 1e-5, "w[1]={}", vals[1]);
    }

    #[test]
    fn compute_scalar_velocity_fields_handle_zero_density() {
        let q = QData {
            rho: vec![1.0, 0.0, -1.0],
            rhou: vec![2.0, 2.0, 2.0],
            rhov: vec![3.0, 3.0, 3.0],
            rhow: vec![4.0, 4.0, 4.0],
            rhoe: vec![0.0; 3],
            gamma: None,
        };
        // Test UVelocity with zero and negative density
        let (u_vals, _, _) = compute_scalar(&q, &crate::plot_state::ScalarField::UVelocity);
        assert_eq!(u_vals.len(), 3);
        assert!((u_vals[0] - 2.0).abs() < 1e-5, "u[0]={}", u_vals[0]);
        assert_eq!(u_vals[1], 0.0, "u[1] should be 0 for zero density");
        assert_eq!(u_vals[2], 0.0, "u[2] should be 0 for negative density");

        // Test VVelocity with zero and negative density
        let (v_vals, _, _) = compute_scalar(&q, &crate::plot_state::ScalarField::VVelocity);
        assert_eq!(v_vals.len(), 3);
        assert!((v_vals[0] - 3.0).abs() < 1e-5, "v[0]={}", v_vals[0]);
        assert_eq!(v_vals[1], 0.0, "v[1] should be 0 for zero density");
        assert_eq!(v_vals[2], 0.0, "v[2] should be 0 for negative density");

        // Test WVelocity with zero and negative density
        let (w_vals, _, _) = compute_scalar(&q, &crate::plot_state::ScalarField::WVelocity);
        assert_eq!(w_vals.len(), 3);
        assert!((w_vals[0] - 4.0).abs() < 1e-5, "w[0]={}", w_vals[0]);
        assert_eq!(w_vals[1], 0.0, "w[1] should be 0 for zero density");
        assert_eq!(w_vals[2], 0.0, "w[2] should be 0 for negative density");
    }
}
