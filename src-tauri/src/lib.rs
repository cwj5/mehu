// Copyright 2026 Charles W Jackson
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

mod com_parser;
mod function_mapping;
mod logger;
mod plot3d;
mod plot_state;
mod script_executor;
mod solution;

#[cfg(test)]
mod logger_tests;

use logger::{clear_logs, export_logs, get_logs, log_debug, log_error, log_info, LogEntry};
use once_cell::sync::Lazy;
use plot3d::{
    extract_contour_lines_from_triangles, get_last_solution_metadata, read_plot3d_function,
    read_plot3d_grid_ascii, read_plot3d_grid_with_metadata, read_plot3d_solution,
    read_plot3d_solution_ascii, GridDimensions, MeshGeometry, Plot3DFunction, Plot3DGrid,
    Plot3DSolution, SolutionFileMetadata,
};
use plot_state::{apply_action, ApplyActionResult, PlotAction, PlotState};
use script_executor::{execute_parsed_script, ScriptExecutionResult};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::webview::WebviewWindow;
use tauri::Emitter;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

// Thread-local storage for solution file metadata
thread_local! {
    static SOLUTION_METADATA: RefCell<Option<SolutionFileMetadata>> = RefCell::new(None);
}

// Deprecated: Legacy solution cache - keeping for backward compatibility during migration
static SOLUTION_CACHE: Lazy<Mutex<Vec<Arc<Plot3DSolution>>>> = Lazy::new(|| Mutex::new(Vec::new()));

/// The canonical shared plot state.  Both script execution and GUI interactions
/// must commit through `apply_plot_action` rather than maintaining separate state.
static PLOT_STATE: Lazy<Mutex<PlotState>> = Lazy::new(|| Mutex::new(PlotState::default()));

fn cache_solutions(solutions: &[Plot3DSolution]) {
    let cached: Vec<Arc<Plot3DSolution>> = solutions
        .iter()
        .map(|solution| Arc::new(solution.clone()))
        .collect();
    if let Ok(mut store) = SOLUTION_CACHE.lock() {
        *store = cached;
    }
}

/// IBLANK filter mode for vertex vs cell mode rendering
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IblankFilterMode {
    Vertex,
    Cell,
}

impl IblankFilterMode {
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "vertex" => Ok(IblankFilterMode::Vertex),
            "cell" => Ok(IblankFilterMode::Cell),
            _ => Err(format!(
                "Invalid IBLANK filter mode: '{}'. Expected 'vertex' or 'cell'",
                s
            )),
        }
    }
}

fn normalize_iblank_flags(
    respect_iblank: Option<bool>,
    show_fringe_points: Option<bool>,
    iblank_filter_mode: Option<String>,
) -> (bool, bool, IblankFilterMode) {
    let effective_respect_iblank = respect_iblank.unwrap_or(false);
    let effective_show_fringe_points = if effective_respect_iblank {
        show_fringe_points.unwrap_or(true)
    } else {
        true
    };
    let effective_filter_mode = iblank_filter_mode
        .as_deref()
        .and_then(|m| IblankFilterMode::from_str(m).ok())
        .unwrap_or(IblankFilterMode::Vertex);

    (
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
    )
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

fn filter_vertex_mode_surface_colors(
    colors: &[f32],
    iblank: Option<&Vec<i32>>,
    grid_i: usize,
    grid_j: usize,
    decimation: usize,
    respect_iblank: bool,
    show_fringe_points: bool,
) -> Option<Vec<f32>> {
    if !respect_iblank || iblank.is_none() {
        return if colors.is_empty() {
            None
        } else {
            Some(colors.to_vec())
        };
    }

    let i_decimated = ((grid_i - 1) / decimation) + 1;
    let j_decimated = ((grid_j - 1) / decimation) + 1;

    let mut filtered_colors = Vec::new();
    for j_step in 0..j_decimated {
        let j_idx = (j_step * decimation).min(grid_j - 1);
        for i_step in 0..i_decimated {
            let i_idx = (i_step * decimation).min(grid_i - 1);
            let grid_idx = j_idx * grid_i + i_idx;
            if is_hidden_iblank_point(iblank, grid_idx, respect_iblank, show_fringe_points) {
                continue;
            }

            let grid_vertex_idx = j_step * i_decimated + i_step;
            let color_idx = grid_vertex_idx * 3;
            if color_idx + 2 < colors.len() {
                filtered_colors.push(colors[color_idx]);
                filtered_colors.push(colors[color_idx + 1]);
                filtered_colors.push(colors[color_idx + 2]);
            }
        }
    }

    if filtered_colors.is_empty() {
        None
    } else {
        Some(filtered_colors)
    }
}

fn compact_mesh_and_colors_to_used_vertices(
    mesh: &mut MeshGeometry,
    colors: &[f32],
) -> Option<Vec<f32>> {
    let old_vertex_count = mesh.vertices.len() / 3;
    if old_vertex_count == 0 {
        mesh.vertex_count = 0;
        mesh.face_count = 0;
        return None;
    }

    let mut used = vec![false; old_vertex_count];
    for &idx in &mesh.indices {
        let uidx = idx as usize;
        if uidx < old_vertex_count {
            used[uidx] = true;
        }
    }
    for &idx in &mesh.triangle_indices {
        let uidx = idx as usize;
        if uidx < old_vertex_count {
            used[uidx] = true;
        }
    }

    if !used.iter().any(|&u| u) {
        mesh.vertices.clear();
        mesh.normals.clear();
        mesh.indices.clear();
        mesh.triangle_indices.clear();
        mesh.vertex_count = 0;
        mesh.face_count = 0;
        return None;
    }

    let mut remap = vec![u32::MAX; old_vertex_count];
    let mut new_vertices = Vec::new();
    let mut new_normals = Vec::new();
    let mut new_colors = Vec::new();

    for old_idx in 0..old_vertex_count {
        if !used[old_idx] {
            continue;
        }

        remap[old_idx] = (new_vertices.len() / 3) as u32;

        let old_vertex_start = old_idx * 3;
        if old_vertex_start + 2 < mesh.vertices.len() {
            new_vertices.push(mesh.vertices[old_vertex_start]);
            new_vertices.push(mesh.vertices[old_vertex_start + 1]);
            new_vertices.push(mesh.vertices[old_vertex_start + 2]);
        }

        if old_vertex_start + 2 < mesh.normals.len() {
            new_normals.push(mesh.normals[old_vertex_start]);
            new_normals.push(mesh.normals[old_vertex_start + 1]);
            new_normals.push(mesh.normals[old_vertex_start + 2]);
        }

        if old_vertex_start + 2 < colors.len() {
            new_colors.push(colors[old_vertex_start]);
            new_colors.push(colors[old_vertex_start + 1]);
            new_colors.push(colors[old_vertex_start + 2]);
        }
    }

    for idx in &mut mesh.indices {
        let old = *idx as usize;
        if old < remap.len() {
            *idx = remap[old];
        }
    }
    for idx in &mut mesh.triangle_indices {
        let old = *idx as usize;
        if old < remap.len() {
            *idx = remap[old];
        }
    }

    mesh.vertices = new_vertices;
    mesh.normals = new_normals;
    mesh.vertex_count = mesh.vertices.len() / 3;
    mesh.face_count = mesh.triangle_indices.len() / 3;

    if new_colors.is_empty() {
        None
    } else {
        Some(new_colors)
    }
}

fn align_surface_mesh_colors(
    mesh: &mut MeshGeometry,
    colors: &[f32],
    iblank: Option<&Vec<i32>>,
    grid_i: usize,
    grid_j: usize,
    decimation: usize,
    respect_iblank: bool,
    show_fringe_points: bool,
    filter_mode: IblankFilterMode,
) -> Option<Vec<f32>> {
    match filter_mode {
        IblankFilterMode::Vertex => filter_vertex_mode_surface_colors(
            colors,
            iblank,
            grid_i,
            grid_j,
            decimation,
            respect_iblank,
            show_fringe_points,
        ),
        IblankFilterMode::Cell => compact_mesh_and_colors_to_used_vertices(mesh, colors),
    }
}

/// Align scalar field values to mesh vertices using the same filtering logic as colors
fn filter_vertex_mode_surface_scalar_values(
    scalar_values: &[f32],
    iblank: Option<&Vec<i32>>,
    grid_i: usize,
    grid_j: usize,
    decimation: usize,
    respect_iblank: bool,
    show_fringe_points: bool,
) -> Option<Vec<f32>> {
    if !respect_iblank || iblank.is_none() {
        return if scalar_values.is_empty() {
            None
        } else {
            Some(scalar_values.to_vec())
        };
    }

    let i_decimated = ((grid_i - 1) / decimation) + 1;
    let j_decimated = ((grid_j - 1) / decimation) + 1;

    let mut filtered_values = Vec::new();
    for j_step in 0..j_decimated {
        let j_idx = (j_step * decimation).min(grid_j - 1);
        for i_step in 0..i_decimated {
            let i_idx = (i_step * decimation).min(grid_i - 1);
            let grid_idx = j_idx * grid_i + i_idx;
            if is_hidden_iblank_point(iblank, grid_idx, respect_iblank, show_fringe_points) {
                continue;
            }

            let grid_vertex_idx = j_step * i_decimated + i_step;
            if grid_vertex_idx < scalar_values.len() {
                filtered_values.push(scalar_values[grid_vertex_idx]);
            }
        }
    }

    if filtered_values.is_empty() {
        None
    } else {
        Some(filtered_values)
    }
}

fn compact_mesh_and_scalar_values_to_used_vertices(
    mesh: &mut MeshGeometry,
    scalar_values: &[f32],
) -> Option<Vec<f32>> {
    let old_vertex_count = mesh.vertices.len() / 3;
    if old_vertex_count == 0 {
        mesh.vertex_count = 0;
        mesh.face_count = 0;
        return None;
    }

    let mut used = vec![false; old_vertex_count];
    for &idx in &mesh.indices {
        let uidx = idx as usize;
        if uidx < old_vertex_count {
            used[uidx] = true;
        }
    }
    for &idx in &mesh.triangle_indices {
        let uidx = idx as usize;
        if uidx < old_vertex_count {
            used[uidx] = true;
        }
    }

    if !used.iter().any(|&u| u) {
        return None;
    }

    let mut new_values = Vec::new();
    for old_idx in 0..old_vertex_count {
        if !used[old_idx] {
            continue;
        }
        if old_idx < scalar_values.len() {
            new_values.push(scalar_values[old_idx]);
        }
    }

    if new_values.is_empty() {
        None
    } else {
        Some(new_values)
    }
}

fn align_surface_mesh_scalar_values(
    mesh: &mut MeshGeometry,
    scalar_values: &[f32],
    iblank: Option<&Vec<i32>>,
    grid_i: usize,
    grid_j: usize,
    decimation: usize,
    respect_iblank: bool,
    show_fringe_points: bool,
    filter_mode: IblankFilterMode,
) -> Option<Vec<f32>> {
    match filter_mode {
        IblankFilterMode::Vertex => filter_vertex_mode_surface_scalar_values(
            scalar_values,
            iblank,
            grid_i,
            grid_j,
            decimation,
            respect_iblank,
            show_fringe_points,
        ),
        IblankFilterMode::Cell => {
            compact_mesh_and_scalar_values_to_used_vertices(mesh, scalar_values)
        }
    }
}

const PROBE_COMPONENT_STRIDE: usize = 6;
const PROBE_IJK_STRIDE: usize = 3;

fn push_probe_components_at(solution: &Plot3DSolution, idx: usize, output: &mut Vec<f32>) {
    const DEFAULT_GAMMA: f32 = 1.4;
    output.push(solution.rho[idx]);
    output.push(solution.rhou[idx]);
    output.push(solution.rhov[idx]);
    output.push(solution.rhow[idx]);
    output.push(solution.rhoe[idx]);
    output.push(
        solution
            .gamma
            .as_ref()
            .and_then(|g| g.get(idx))
            .copied()
            .unwrap_or(DEFAULT_GAMMA),
    );
}

fn build_surface_probe_components(
    solution: &Plot3DSolution,
    grid_i: usize,
    grid_j: usize,
    decimation: usize,
) -> Vec<f32> {
    let i_decimated = ((grid_i - 1) / decimation) + 1;
    let j_decimated = ((grid_j - 1) / decimation) + 1;
    let mut probe_components =
        Vec::with_capacity(i_decimated * j_decimated * PROBE_COMPONENT_STRIDE);

    for j_step in 0..j_decimated {
        let j_idx = (j_step * decimation).min(grid_j - 1);
        for i_step in 0..i_decimated {
            let i_idx = (i_step * decimation).min(grid_i - 1);
            let grid_idx = j_idx * grid_i + i_idx;
            push_probe_components_at(solution, grid_idx, &mut probe_components);
        }
    }

    probe_components
}

fn filter_vertex_mode_surface_probe_components(
    probe_components: &[f32],
    iblank: Option<&Vec<i32>>,
    grid_i: usize,
    grid_j: usize,
    decimation: usize,
    respect_iblank: bool,
    show_fringe_points: bool,
) -> Option<Vec<f32>> {
    if !respect_iblank || iblank.is_none() {
        return if probe_components.is_empty() {
            None
        } else {
            Some(probe_components.to_vec())
        };
    }

    let i_decimated = ((grid_i - 1) / decimation) + 1;
    let j_decimated = ((grid_j - 1) / decimation) + 1;

    let mut filtered = Vec::new();
    for j_step in 0..j_decimated {
        let j_idx = (j_step * decimation).min(grid_j - 1);
        for i_step in 0..i_decimated {
            let i_idx = (i_step * decimation).min(grid_i - 1);
            let grid_idx = j_idx * grid_i + i_idx;
            if is_hidden_iblank_point(iblank, grid_idx, respect_iblank, show_fringe_points) {
                continue;
            }

            let grid_vertex_idx = j_step * i_decimated + i_step;
            let start = grid_vertex_idx * PROBE_COMPONENT_STRIDE;
            if start + (PROBE_COMPONENT_STRIDE - 1) < probe_components.len() {
                filtered
                    .extend_from_slice(&probe_components[start..start + PROBE_COMPONENT_STRIDE]);
            }
        }
    }

    if filtered.is_empty() {
        None
    } else {
        Some(filtered)
    }
}

fn compact_mesh_and_probe_components_to_used_vertices(
    mesh: &mut MeshGeometry,
    probe_components: &[f32],
) -> Option<Vec<f32>> {
    let old_vertex_count = mesh.vertices.len() / 3;
    if old_vertex_count == 0 {
        mesh.vertex_count = 0;
        mesh.face_count = 0;
        return None;
    }

    let mut used = vec![false; old_vertex_count];
    for &idx in &mesh.indices {
        let uidx = idx as usize;
        if uidx < old_vertex_count {
            used[uidx] = true;
        }
    }
    for &idx in &mesh.triangle_indices {
        let uidx = idx as usize;
        if uidx < old_vertex_count {
            used[uidx] = true;
        }
    }

    if !used.iter().any(|&u| u) {
        return None;
    }

    let mut compacted = Vec::new();
    for old_idx in 0..old_vertex_count {
        if !used[old_idx] {
            continue;
        }

        let start = old_idx * PROBE_COMPONENT_STRIDE;
        if start + (PROBE_COMPONENT_STRIDE - 1) < probe_components.len() {
            compacted.extend_from_slice(&probe_components[start..start + PROBE_COMPONENT_STRIDE]);
        }
    }

    if compacted.is_empty() {
        None
    } else {
        Some(compacted)
    }
}

fn align_surface_mesh_probe_components(
    mesh: &mut MeshGeometry,
    probe_components: &[f32],
    iblank: Option<&Vec<i32>>,
    grid_i: usize,
    grid_j: usize,
    decimation: usize,
    respect_iblank: bool,
    show_fringe_points: bool,
    filter_mode: IblankFilterMode,
) -> Option<Vec<f32>> {
    match filter_mode {
        IblankFilterMode::Vertex => filter_vertex_mode_surface_probe_components(
            probe_components,
            iblank,
            grid_i,
            grid_j,
            decimation,
            respect_iblank,
            show_fringe_points,
        ),
        IblankFilterMode::Cell => {
            compact_mesh_and_probe_components_to_used_vertices(mesh, probe_components)
        }
    }
}

fn push_probe_ijk(i: usize, j: usize, k: usize, output: &mut Vec<u32>) {
    output.push((i + 1) as u32);
    output.push((j + 1) as u32);
    output.push((k + 1) as u32);
}

fn linear_index_to_ijk(idx: usize, dim_i: usize, dim_j: usize) -> (usize, usize, usize) {
    let i = idx % dim_i;
    let j = (idx / dim_i) % dim_j;
    let k = idx / (dim_i * dim_j);
    (i, j, k)
}

fn build_surface_probe_ijk(grid_i: usize, grid_j: usize, decimation: usize) -> Vec<u32> {
    let i_decimated = ((grid_i - 1) / decimation) + 1;
    let j_decimated = ((grid_j - 1) / decimation) + 1;
    let mut probe_ijk = Vec::with_capacity(i_decimated * j_decimated * PROBE_IJK_STRIDE);

    for j_step in 0..j_decimated {
        let j_idx = (j_step * decimation).min(grid_j - 1);
        for i_step in 0..i_decimated {
            let i_idx = (i_step * decimation).min(grid_i - 1);
            push_probe_ijk(i_idx, j_idx, 0, &mut probe_ijk);
        }
    }

    probe_ijk
}

fn filter_vertex_mode_surface_probe_ijk(
    probe_ijk: &[u32],
    iblank: Option<&Vec<i32>>,
    grid_i: usize,
    grid_j: usize,
    decimation: usize,
    respect_iblank: bool,
    show_fringe_points: bool,
) -> Option<Vec<u32>> {
    if !respect_iblank || iblank.is_none() {
        return if probe_ijk.is_empty() {
            None
        } else {
            Some(probe_ijk.to_vec())
        };
    }

    let i_decimated = ((grid_i - 1) / decimation) + 1;
    let j_decimated = ((grid_j - 1) / decimation) + 1;

    let mut filtered = Vec::new();
    for j_step in 0..j_decimated {
        let j_idx = (j_step * decimation).min(grid_j - 1);
        for i_step in 0..i_decimated {
            let i_idx = (i_step * decimation).min(grid_i - 1);
            let grid_idx = j_idx * grid_i + i_idx;
            if is_hidden_iblank_point(iblank, grid_idx, respect_iblank, show_fringe_points) {
                continue;
            }

            let grid_vertex_idx = j_step * i_decimated + i_step;
            let start = grid_vertex_idx * PROBE_IJK_STRIDE;
            if start + (PROBE_IJK_STRIDE - 1) < probe_ijk.len() {
                filtered.extend_from_slice(&probe_ijk[start..start + PROBE_IJK_STRIDE]);
            }
        }
    }

    if filtered.is_empty() {
        None
    } else {
        Some(filtered)
    }
}

fn compact_mesh_and_probe_ijk_to_used_vertices(
    mesh: &mut MeshGeometry,
    probe_ijk: &[u32],
) -> Option<Vec<u32>> {
    let old_vertex_count = mesh.vertices.len() / 3;
    if old_vertex_count == 0 {
        mesh.vertex_count = 0;
        mesh.face_count = 0;
        return None;
    }

    let mut used = vec![false; old_vertex_count];
    for &idx in &mesh.indices {
        let uidx = idx as usize;
        if uidx < old_vertex_count {
            used[uidx] = true;
        }
    }
    for &idx in &mesh.triangle_indices {
        let uidx = idx as usize;
        if uidx < old_vertex_count {
            used[uidx] = true;
        }
    }

    if !used.iter().any(|&u| u) {
        return None;
    }

    let mut compacted = Vec::new();
    for old_idx in 0..old_vertex_count {
        if !used[old_idx] {
            continue;
        }

        let start = old_idx * PROBE_IJK_STRIDE;
        if start + (PROBE_IJK_STRIDE - 1) < probe_ijk.len() {
            compacted.extend_from_slice(&probe_ijk[start..start + PROBE_IJK_STRIDE]);
        }
    }

    if compacted.is_empty() {
        None
    } else {
        Some(compacted)
    }
}

fn align_surface_mesh_probe_ijk(
    mesh: &mut MeshGeometry,
    probe_ijk: &[u32],
    iblank: Option<&Vec<i32>>,
    grid_i: usize,
    grid_j: usize,
    decimation: usize,
    respect_iblank: bool,
    show_fringe_points: bool,
    filter_mode: IblankFilterMode,
) -> Option<Vec<u32>> {
    match filter_mode {
        IblankFilterMode::Vertex => filter_vertex_mode_surface_probe_ijk(
            probe_ijk,
            iblank,
            grid_i,
            grid_j,
            decimation,
            respect_iblank,
            show_fringe_points,
        ),
        IblankFilterMode::Cell => compact_mesh_and_probe_ijk_to_used_vertices(mesh, probe_ijk),
    }
}

#[cfg(test)]
mod iblank_flag_tests {
    use super::{
        align_surface_mesh_colors, normalize_iblank_flags, IblankFilterMode, MeshGeometry,
    };

    #[test]
    fn normalize_defaults_to_no_respect_and_show_fringe() {
        let (respect, show_fringe, mode) = normalize_iblank_flags(None, None, None);
        assert!(!respect);
        assert!(show_fringe);
        assert_eq!(mode, IblankFilterMode::Vertex);
    }

    #[test]
    fn normalize_forces_show_fringe_when_not_respecting_iblank() {
        let (respect, show_fringe, mode) = normalize_iblank_flags(Some(false), Some(false), None);
        assert!(!respect);
        assert!(show_fringe);
        assert_eq!(mode, IblankFilterMode::Vertex);
    }

    #[test]
    fn normalize_preserves_show_fringe_when_respecting_iblank() {
        let (respect, show_fringe, mode) = normalize_iblank_flags(Some(true), Some(false), None);
        assert!(respect);
        assert!(!show_fringe);
        assert_eq!(mode, IblankFilterMode::Vertex);
    }

    #[test]
    fn normalize_parses_vertex_mode_correctly() {
        let (_, _, mode) = normalize_iblank_flags(None, None, Some("vertex".to_string()));
        assert_eq!(mode, IblankFilterMode::Vertex);
    }

    #[test]
    fn normalize_parses_cell_mode_correctly() {
        let (_, _, mode) = normalize_iblank_flags(None, None, Some("cell".to_string()));
        assert_eq!(mode, IblankFilterMode::Cell);
    }

    #[test]
    fn normalize_defaults_to_vertex_mode_on_invalid() {
        let (_, _, mode) = normalize_iblank_flags(None, None, Some("invalid_mode".to_string()));
        assert_eq!(mode, IblankFilterMode::Vertex); // Falls back to default
    }

    #[test]
    fn align_vertex_mode_filters_hidden_fringe_colors() {
        let mut mesh = MeshGeometry {
            vertices: vec![],
            indices: vec![],
            triangle_indices: vec![],
            normals: vec![],
            vertex_count: 0,
            face_count: 0,
            colors: None,
            scalar_values: None,
            probe_components: None,
            probe_ijk: None,
            vertex_cell_data: None,
        };

        let colors = vec![
            1.0, 0.0, 0.0, // v0
            0.0, 1.0, 0.0, // v1 (fringe, should be removed)
            0.0, 0.0, 1.0, // v2
            1.0, 1.0, 0.0, // v3
        ];
        let iblank = vec![1, -1, 1, 1];

        let aligned = align_surface_mesh_colors(
            &mut mesh,
            &colors,
            Some(&iblank),
            2,
            2,
            1,
            true,
            false,
            IblankFilterMode::Vertex,
        )
        .expect("Expected filtered colors");

        assert_eq!(aligned.len(), 9);
        assert_eq!(aligned, vec![1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0]);
    }

    #[test]
    fn align_cell_mode_compacts_mesh_and_colors_to_used_vertices() {
        let mut mesh = MeshGeometry {
            vertices: vec![
                0.0, 0.0, 0.0, // v0 (used)
                1.0, 0.0, 0.0, // v1 (used)
                1.0, 1.0, 0.0, // v2 (used)
                0.0, 1.0, 0.0, // v3 (unused)
            ],
            indices: vec![0, 1, 1, 2, 2, 0],
            triangle_indices: vec![0, 1, 2],
            normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            vertex_count: 4,
            face_count: 1,
            colors: None,
            scalar_values: None,
            probe_components: None,
            probe_ijk: None,
            vertex_cell_data: None,
        };

        let colors = vec![
            1.0, 0.0, 0.0, // v0
            0.0, 1.0, 0.0, // v1
            0.0, 0.0, 1.0, // v2
            1.0, 1.0, 0.0, // v3 (unused, should be dropped)
        ];

        let aligned = align_surface_mesh_colors(
            &mut mesh,
            &colors,
            None,
            2,
            2,
            1,
            true,
            true,
            IblankFilterMode::Cell,
        )
        .expect("Expected compacted colors");

        assert_eq!(mesh.vertex_count, 3);
        assert_eq!(mesh.vertices.len(), 9);
        assert_eq!(mesh.normals.len(), 9);
        assert_eq!(mesh.indices, vec![0, 1, 1, 2, 2, 0]);
        assert_eq!(mesh.triangle_indices, vec![0, 1, 2]);
        assert_eq!(aligned.len(), 9);
        assert_eq!(aligned, vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
    }
}

// ============================================================================
// NEW: Grid and Solution Cache Architecture
// ============================================================================

/// Cached grid entry with metadata
#[derive(Clone, Debug, Serialize)]
struct CachedGrid {
    id: String,
    grid: Arc<Plot3DGrid>,
    file_path: String,
    file_name: String,
    grid_index: usize,
    has_iblank: bool,
}

/// Cached solution entry with metadata
#[derive(Clone, Debug, Serialize)]
struct CachedSolution {
    id: String,
    solution: Arc<Plot3DSolution>,
    file_path: String,
    file_name: String,
    grid_index: usize,
}

/// Metadata about a cached grid (no coordinate arrays)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GridMetadata {
    pub id: String,
    pub file_path: String,
    pub file_name: String,
    pub grid_index: usize,
    pub dimensions: GridDimensions,
    pub has_iblank: bool,
    pub has_solution: bool,
}

/// Metadata about a cached solution (no arrays)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SolutionMetadata {
    pub id: String,
    pub file_path: String,
    pub file_name: String,
    pub grid_index: usize,
    pub dimensions: GridDimensions,
}

/// Global grid cache: grid_id -> CachedGrid
static GRID_CACHE: Lazy<Mutex<HashMap<String, CachedGrid>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Global solution cache: solution_id -> CachedSolution
static SOLUTION_CACHE_V2: Lazy<Mutex<HashMap<String, CachedSolution>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Clone)]
struct CachedArbitraryPlaneField {
    vertices: Arc<Vec<f32>>,
    triangle_indices: Arc<Vec<u32>>,
    scalar_values: Arc<Vec<f32>>,
}

static ARBITRARY_PLANE_FIELD_CACHE: Lazy<Mutex<HashMap<String, CachedArbitraryPlaneField>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Counter for generating unique grid IDs
static GRID_ID_COUNTER: Lazy<Mutex<u64>> = Lazy::new(|| Mutex::new(0));

/// Generate a unique grid ID
fn generate_grid_id(file_path: &str, grid_index: usize) -> String {
    let mut counter = GRID_ID_COUNTER.lock().unwrap();
    *counter += 1;
    format!(
        "grid_{}_{}_idx{}_{}",
        *counter,
        Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown"),
        grid_index,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            % 100000
    )
}

/// Generate a unique solution ID
fn generate_solution_id(file_path: &str, grid_index: usize) -> String {
    let mut counter = GRID_ID_COUNTER.lock().unwrap();
    *counter += 1;
    format!(
        "solution_{}_{}_idx{}_{}",
        *counter,
        Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown"),
        grid_index,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            % 100000
    )
}

fn quantize_plane_component(v: f32) -> i64 {
    // 1e-6 quantization keeps cache keys stable against small float jitter.
    (v as f64 * 1_000_000.0).round() as i64
}

fn arbitrary_plane_field_cache_key(
    grid_id: &str,
    solution_id: &str,
    plane_point: [f32; 3],
    plane_normal: [f32; 3],
    scalar_field: &str,
    respect_iblank: bool,
    show_fringe_points: bool,
    iblank_filter_mode: IblankFilterMode,
) -> String {
    let mode = match iblank_filter_mode {
        IblankFilterMode::Vertex => "vertex",
        IblankFilterMode::Cell => "cell",
    };
    format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        grid_id,
        solution_id,
        scalar_field,
        quantize_plane_component(plane_point[0]),
        quantize_plane_component(plane_point[1]),
        quantize_plane_component(plane_point[2]),
        quantize_plane_component(plane_normal[0]),
        quantize_plane_component(plane_normal[1]),
        quantize_plane_component(plane_normal[2]),
        if respect_iblank { 1 } else { 0 },
        if show_fringe_points { 1 } else { 0 },
        mode,
        "v1"
    )
}

fn get_or_build_arbitrary_plane_field_sample(
    grid: &Plot3DGrid,
    solution: &Plot3DSolution,
    scalar_field: plot_state::ScalarField,
    plane_point: [f32; 3],
    plane_normal: [f32; 3],
    respect_iblank: bool,
    show_fringe_points: bool,
    iblank_filter_mode: IblankFilterMode,
    cache_key: &str,
) -> Result<CachedArbitraryPlaneField, String> {
    if let Ok(cache) = ARBITRARY_PLANE_FIELD_CACHE.lock() {
        if let Some(found) = cache.get(cache_key) {
            return Ok(found.clone());
        }
    }

    let (vertices, triangle_indices, scalar_values) = grid.interpolate_arbitrary_plane_field_data(
        solution,
        plane_point,
        plane_normal,
        scalar_field,
        respect_iblank,
        show_fringe_points,
        iblank_filter_mode,
    )?;

    let sample = CachedArbitraryPlaneField {
        vertices: Arc::new(vertices),
        triangle_indices: Arc::new(triangle_indices),
        scalar_values: Arc::new(scalar_values),
    };

    if let Ok(mut cache) = ARBITRARY_PLANE_FIELD_CACHE.lock() {
        if cache.len() > 128 {
            cache.clear();
        }
        cache.insert(cache_key.to_string(), sample.clone());
    }

    Ok(sample)
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// Load PLOT3D grid file (auto-detects binary format)
#[tauri::command]
fn load_plot3d_file(path: String) -> Result<Vec<Plot3DGrid>, String> {
    match read_plot3d_grid_with_metadata(&path) {
        Ok((grids, metadata)) => {
            let dims_str = metadata
                .grid_dimensions
                .iter()
                .enumerate()
                .map(|(idx, d)| format!("Grid {} ({}×{}×{})", idx + 1, d.i, d.j, d.k))
                .collect::<Vec<_>>()
                .join(", ");

            log_info(&format!(
                "Loaded grid file {} (endianness: {}, precision: {}, iblank: {})",
                path,
                metadata.byte_order,
                metadata.precision,
                if metadata.has_iblank { "yes" } else { "no" }
            ));
            log_info(&format!("Grids: {}", dims_str));

            Ok(grids)
        }
        Err(e) => {
            let error_msg = format!("Error loading PLOT3D file: {}", e);
            log_error(&error_msg);
            Err(error_msg)
        }
    }
}

/// Load PLOT3D grid file in ASCII format
#[tauri::command]
fn load_plot3d_file_ascii(path: String) -> Result<Vec<Plot3DGrid>, String> {
    match read_plot3d_grid_ascii(&path) {
        Ok(grids) => {
            let dims_str = grids
                .iter()
                .enumerate()
                .map(|(idx, d)| {
                    format!(
                        "Grid {} ({}×{}×{})",
                        idx, d.dimensions.i, d.dimensions.j, d.dimensions.k
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");

            log_info(&format!(
                "Loaded ASCII grid file {} (endianness: ASCII, precision: f32, iblank: no)",
                path
            ));
            log_info(&format!("Grids: {}", dims_str));
            Ok(grids)
        }
        Err(e) => {
            let error_msg = format!("Error loading ASCII PLOT3D file: {}", e);
            log_error(&error_msg);
            Err(error_msg)
        }
    }
}

/// Load PLOT3D solution file (Q file) in binary format
#[tauri::command]
fn load_plot3d_solution(path: String) -> Result<Vec<Plot3DSolution>, String> {
    log_debug(&format!("Loading PLOT3D solution file: {}", path));
    match read_plot3d_solution(&path) {
        Ok(solutions) => {
            cache_solutions(&solutions);
            // Get the metadata that was set by the reader
            if let Some(metadata) = get_last_solution_metadata() {
                log_info(&format!(
                    "Loaded solution file {} ({} format, {} precision, endianness: {})",
                    path, metadata.format, metadata.precision, metadata.byte_order
                ));
            } else {
                log_info(&format!(
                    "Successfully loaded {} solution(s) from {} (binary format)",
                    solutions.len(),
                    path
                ));
            }
            Ok(solutions)
        }
        Err(e) => {
            let error_msg = format!("Error loading PLOT3D solution file: {}", e);
            log_error(&error_msg);
            Err(error_msg)
        }
    }
}

/// Load PLOT3D solution file (Q file) in ASCII format
#[tauri::command]
fn load_plot3d_solution_ascii(path: String) -> Result<Vec<Plot3DSolution>, String> {
    log_debug(&format!("Loading ASCII PLOT3D solution file: {}", path));
    match read_plot3d_solution_ascii(&path) {
        Ok(solutions) => {
            cache_solutions(&solutions);
            // Get the metadata that was set by the reader
            if let Some(metadata) = get_last_solution_metadata() {
                log_info(&format!(
                    "Loaded solution file {} ({} format, {} precision)",
                    path, metadata.format, metadata.precision
                ));
            } else {
                log_info(&format!(
                    "Successfully loaded {} solution(s) from {} (ASCII format)",
                    solutions.len(),
                    path
                ));
            }
            Ok(solutions)
        }
        Err(e) => {
            let error_msg = format!("Error loading ASCII PLOT3D solution file: {}", e);
            log_error(&error_msg);
            Err(error_msg)
        }
    }
}

/// Load PLOT3D solution file (Q file) - auto-detects binary or ASCII format
#[tauri::command]
fn load_plot3d_solution_auto(path: String) -> Result<Vec<Plot3DSolution>, String> {
    log_debug(&format!(
        "Loading PLOT3D solution file (auto-detect): {}",
        path
    ));

    // First, check file size and basic properties
    use std::fs;
    let metadata = fs::metadata(&path).map_err(|e| format!("Failed to read file: {}", e))?;

    log_debug(&format!("File size: {} bytes", metadata.len()));

    if metadata.len() == 0 {
        return Err("Solution file is empty".to_string());
    }

    // Try to detect file type by reading first few bytes
    let file_bytes = fs::read(&path).map_err(|e| format!("Failed to read file: {}", e))?;

    let is_likely_text = file_bytes
        .iter()
        .take(500)
        .all(|&b| b == b'\n' || b == b'\r' || b == b'\t' || (b >= 32 && b < 127));

    log_debug(&format!(
        "File appears to be: {}",
        if is_likely_text {
            "text (ASCII)"
        } else {
            "binary"
        }
    ));

    // Try binary format first (more specific format)
    match read_plot3d_solution(&path) {
        Ok(solutions) => {
            cache_solutions(&solutions);
            // Get the metadata that was set by the reader
            if let Some(metadata) = get_last_solution_metadata() {
                log_info(&format!(
                    "Loaded solution file {} ({} format, {} precision, endianness: {})",
                    path, metadata.format, metadata.precision, metadata.byte_order
                ));
            } else {
                log_info(&format!(
                    "Successfully loaded {} solution(s) from {} (binary format)",
                    solutions.len(),
                    path
                ));
            }
            Ok(solutions)
        }
        Err(binary_err) => {
            // Binary failed, try ASCII format
            log_debug(&format!("Binary format failed: {}", binary_err));
            match read_plot3d_solution_ascii(&path) {
                Ok(solutions) => {
                    cache_solutions(&solutions);
                    // Get the metadata that was set by the reader
                    if let Some(metadata) = get_last_solution_metadata() {
                        log_info(&format!(
                            "Loaded solution file {} ({} format, {} precision)",
                            path, metadata.format, metadata.precision
                        ));
                    } else {
                        log_info(&format!(
                            "Successfully loaded {} solution(s) from {} (ASCII format)",
                            solutions.len(),
                            path
                        ));
                    }
                    Ok(solutions)
                }
                Err(ascii_err) => {
                    log_debug(&format!("ASCII format failed: {}", ascii_err));
                    let file_type = if is_likely_text {
                        "text file"
                    } else {
                        "binary file"
                    };
                    let error_msg = format!(
                        "Failed to load solution file (detected as {}). Binary reader: {}. ASCII reader: {}",
                        file_type, binary_err, ascii_err
                    );
                    log_error(&error_msg);
                    Err(error_msg)
                }
            }
        }
    }
}

/// Load PLOT3D function file (F file) in binary format
#[tauri::command]
fn load_plot3d_function(path: String) -> Result<Vec<Plot3DFunction>, String> {
    log_debug(&format!("Loading PLOT3D function file: {}", path));
    match read_plot3d_function(&path) {
        Ok(functions) => {
            log_info(&format!(
                "Successfully loaded {} function file(s) from {}",
                functions.len(),
                path
            ));
            Ok(functions)
        }
        Err(e) => {
            let error_msg = format!("Error loading PLOT3D function file: {}", e);
            log_error(&error_msg);
            Err(error_msg)
        }
    }
}

// ============================================================================
// NEW: V2 Load Commands that Cache and Return Metadata
// ============================================================================

/// Load PLOT3D grid file (caches grids and returns metadata)
#[tauri::command]
fn load_plot3d_file_cached(path: String) -> Result<Vec<GridMetadata>, String> {
    // Load grids with binary-first auto-fallback to ASCII.
    let (grids, byte_order, precision, has_iblank) = match read_plot3d_grid_with_metadata(&path) {
        Ok((grids, file_metadata)) => (
            grids,
            file_metadata.byte_order,
            file_metadata.precision,
            file_metadata.has_iblank,
        ),
        Err(binary_err) => match read_plot3d_grid_ascii(&path) {
            Ok(grids) => {
                let has_iblank = grids.iter().any(|g| g.iblank.is_some());
                (
                    grids,
                    "N/A (ASCII)".to_string(),
                    "f32".to_string(),
                    has_iblank,
                )
            }
            Err(ascii_err) => {
                let error_msg = format!(
                    "Error loading PLOT3D file. Binary: {}. ASCII: {}",
                    binary_err, ascii_err
                );
                log_error(&error_msg);
                return Err(error_msg);
            }
        },
    };

    let file_name = Path::new(&path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let dims_str = grids
        .iter()
        .enumerate()
        .map(|(idx, g)| {
            format!(
                "Grid {} ({}×{}×{})",
                idx + 1,
                g.dimensions.i,
                g.dimensions.j,
                g.dimensions.k
            )
        })
        .collect::<Vec<_>>()
        .join(", ");

    log_info(&format!(
        "Loaded grid file {} (endianness: {}, precision: {}, iblank: {})",
        path,
        byte_order,
        precision,
        if has_iblank { "yes" } else { "no" }
    ));
    log_info(&format!("Grids: {}", dims_str));

    // Cache grids and generate metadata
    let mut cache = GRID_CACHE
        .lock()
        .map_err(|_| "Grid cache lock poisoned".to_string())?;

    let mut metadata_list = Vec::new();

    for (grid_index, grid) in grids.into_iter().enumerate() {
        let grid_id = generate_grid_id(&path, grid_index);
        let has_iblank = grid.iblank.is_some();

        let cached_grid = CachedGrid {
            id: grid_id.clone(),
            grid: Arc::new(grid.clone()),
            file_path: path.clone(),
            file_name: file_name.clone(),
            grid_index,
            has_iblank,
        };

        cache.insert(grid_id.clone(), cached_grid);

        metadata_list.push(GridMetadata {
            id: grid_id,
            file_path: path.clone(),
            file_name: file_name.clone(),
            grid_index,
            dimensions: grid.dimensions,
            has_iblank,
            has_solution: false, // Will be updated when solution is loaded
        });
    }

    log_info(&format!("Cached {} grids", metadata_list.len()));

    Ok(metadata_list)
}

/// Load PLOT3D solution file (caches solutions and returns metadata)
#[tauri::command]
fn load_plot3d_solution_cached(path: String) -> Result<Vec<SolutionMetadata>, String> {
    log_debug(&format!("Loading PLOT3D solution file (v2): {}", path));

    // Load solutions using existing reader (auto-detects format)
    let (solutions, _) = {
        // Try binary first
        match read_plot3d_solution(&path) {
            Ok(solutions) => {
                let metadata = get_last_solution_metadata();
                (solutions, metadata)
            }
            Err(binary_err) => {
                // Try ASCII
                match read_plot3d_solution_ascii(&path) {
                    Ok(solutions) => {
                        let metadata = get_last_solution_metadata();
                        (solutions, metadata)
                    }
                    Err(ascii_err) => {
                        let error_msg = format!(
                            "Failed to load solution file. Binary: {}. ASCII: {}",
                            binary_err, ascii_err
                        );
                        log_error(&error_msg);
                        return Err(error_msg);
                    }
                }
            }
        }
    };

    let file_name = Path::new(&path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    if let Some(metadata) = get_last_solution_metadata() {
        log_info(&format!(
            "Loaded solution file {} ({} format, {} precision, endianness: {})",
            path, metadata.format, metadata.precision, metadata.byte_order
        ));
    } else {
        log_info(&format!(
            "Successfully loaded {} solution(s) from {}",
            solutions.len(),
            path
        ));
    }

    // Cache solutions for old API compatibility
    cache_solutions(&solutions);

    // Cache solutions in v2 cache and generate metadata
    let mut cache = SOLUTION_CACHE_V2
        .lock()
        .map_err(|_| "Solution cache lock poisoned".to_string())?;

    let mut metadata_list = Vec::new();

    for solution in solutions.into_iter() {
        let grid_index = solution.grid_index;
        let solution_id = generate_solution_id(&path, grid_index);

        let cached_solution = CachedSolution {
            id: solution_id.clone(),
            solution: Arc::new(solution.clone()),
            file_path: path.clone(),
            file_name: file_name.clone(),
            grid_index,
        };

        cache.insert(solution_id.clone(), cached_solution);

        metadata_list.push(SolutionMetadata {
            id: solution_id,
            file_path: path.clone(),
            file_name: file_name.clone(),
            grid_index,
            dimensions: solution.dimensions,
        });
    }

    log_info(&format!("Cached {} solutions", metadata_list.len()));

    Ok(metadata_list)
}

/// Convert PLOT3D grid to Three.js mesh geometry
#[tauri::command]
fn convert_grid_to_mesh(
    grid: Plot3DGrid,
    respect_iblank: Option<bool>,
    show_fringe_points: Option<bool>,
    iblank_filter_mode: Option<String>,
    window: WebviewWindow,
) -> Result<MeshGeometry, String> {
    let (effective_respect_iblank, effective_show_fringe_points, effective_filter_mode) =
        normalize_iblank_flags(respect_iblank, show_fringe_points, iblank_filter_mode);

    // Emit loading start event
    let _ = window.emit("loading-start", "Converting grid to mesh...");

    // Validate grid data
    let total_points = grid.total_points();
    if grid.x_coords.len() != total_points {
        let error_msg = format!(
            "Invalid grid: x_coords length {} != expected {} ({}x{}x{})",
            grid.x_coords.len(),
            total_points,
            grid.dimensions.i,
            grid.dimensions.j,
            grid.dimensions.k
        );
        log_error(&error_msg);
        return Err(error_msg);
    }
    if grid.y_coords.len() != total_points {
        let error_msg = format!(
            "Invalid grid: y_coords length {} != expected {}",
            grid.y_coords.len(),
            total_points
        );
        log_error(&error_msg);
        return Err(error_msg);
    }
    if grid.z_coords.len() != total_points {
        let error_msg = format!(
            "Invalid grid: z_coords length {} != expected {}",
            grid.z_coords.len(),
            total_points
        );
        log_error(&error_msg);
        return Err(error_msg);
    }

    // Auto-detect decimation based on grid size for better performance
    let i = grid.dimensions.i as usize;
    let j = grid.dimensions.j as usize;
    let max_dim = i.max(j);

    let decimation_factor = if max_dim > 1000 {
        4 // Very large grids: use 1/4 resolution
    } else if max_dim > 500 {
        3 // Large grids: use 1/3 resolution
    } else if max_dim > 250 {
        2 // Medium grids: use 1/2 resolution
    } else {
        1 // Small grids: full resolution
    };

    if decimation_factor > 1 {
        log_info(&format!(
            "Grid size {}x{} - applying {}x decimation for performance",
            i, j, decimation_factor
        ));
    }

    let mesh = grid.to_mesh_surface_geometry_decimated(
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
        decimation_factor,
    );

    // Emit loading end event
    let _ = window.emit("loading-end", ());

    Ok(mesh)
}

// ============================================================================
// NEW: ID-Based Compute Commands (Phase 2)
// ============================================================================

/// Convert cached grid to mesh geometry (ID-based)
#[allow(non_snake_case)]
#[tauri::command]
fn convert_grid_to_mesh_by_id(
    gridId: String,
    respect_iblank: Option<bool>,
    show_fringe_points: Option<bool>,
    iblank_filter_mode: Option<String>,
    window: WebviewWindow,
) -> Result<MeshGeometry, String> {
    let _ = window.emit("loading-start", "Converting grid to mesh...");

    let (effective_respect_iblank, effective_show_fringe_points, effective_filter_mode) =
        normalize_iblank_flags(respect_iblank, show_fringe_points, iblank_filter_mode);

    // Load grid from cache
    let grid = {
        let cache = GRID_CACHE
            .lock()
            .map_err(|_| "Grid cache lock poisoned".to_string())?;
        let cached = cache
            .get(&gridId)
            .ok_or_else(|| format!("Grid not found in cache: {}", gridId))?;
        Arc::clone(&cached.grid)
    };

    // Validate grid data
    let total_points = grid.total_points();
    if grid.x_coords.len() != total_points {
        return Err(format!(
            "Invalid grid: x_coords length {} != expected {}",
            grid.x_coords.len(),
            total_points
        ));
    }

    // Auto-detect decimation
    let i = grid.dimensions.i as usize;
    let j = grid.dimensions.j as usize;
    let max_dim = i.max(j);

    let decimation_factor = if max_dim > 1000 {
        4
    } else if max_dim > 500 {
        3
    } else if max_dim > 250 {
        2
    } else {
        1
    };

    if decimation_factor > 1 {
        log_info(&format!(
            "Grid size {}x{} - applying {}x decimation for performance",
            i, j, decimation_factor
        ));
    }

    let mesh = grid.to_mesh_surface_geometry_decimated(
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
        decimation_factor,
    );

    let _ = window.emit("loading-end", ());

    Ok(mesh)
}

/// Slice a cached grid along I/J/K plane (ID-based)
#[allow(non_snake_case)]
#[tauri::command]
fn slice_grid_by_id(gridId: String, plane: String, index: u32) -> Result<Plot3DGrid, String> {
    log_debug(&format!(
        "Slicing cached grid {} along {} plane at index {}",
        gridId, plane, index
    ));

    // Load grid from cache
    let grid = {
        let cache = GRID_CACHE
            .lock()
            .map_err(|_| "Grid cache lock poisoned".to_string())?;
        let cached = cache
            .get(&gridId)
            .ok_or_else(|| format!("Grid not found in cache: {}", gridId))?;
        Arc::clone(&cached.grid)
    };

    grid.slice_grid(&plane, index).map_err(|e| {
        let error_msg = format!("Failed to slice grid: {}", e);
        log_error(&error_msg);
        error_msg
    })
}

fn normalize_subset_bounds(start: Option<u32>, end: Option<u32>, dim: u32) -> (usize, usize) {
    let max = dim.max(1);
    let mut s = start.unwrap_or(1).clamp(1, max);
    let mut e = end.unwrap_or(max).clamp(1, max);
    if s > e {
        std::mem::swap(&mut s, &mut e);
    }
    ((s - 1) as usize, (e - 1) as usize)
}

fn build_subset_grid(
    grid: &Plot3DGrid,
    i_start: Option<u32>,
    i_end: Option<u32>,
    j_start: Option<u32>,
    j_end: Option<u32>,
    k_start: Option<u32>,
    k_end: Option<u32>,
) -> Result<(Plot3DGrid, Vec<usize>), String> {
    let ni = grid.dimensions.i;
    let nj = grid.dimensions.j;
    let nk = grid.dimensions.k;

    if ni == 0 || nj == 0 || nk == 0 {
        return Err("Grid dimensions must be non-zero".to_string());
    }

    let (i0, i1) = normalize_subset_bounds(i_start, i_end, ni);
    let (j0, j1) = normalize_subset_bounds(j_start, j_end, nj);
    let (k0, k1) = normalize_subset_bounds(k_start, k_end, nk);

    let out_i = i1 - i0 + 1;
    let out_j = j1 - j0 + 1;
    let out_k = k1 - k0 + 1;
    let out_points = out_i * out_j * out_k;

    let mut x_coords = Vec::with_capacity(out_points);
    let mut y_coords = Vec::with_capacity(out_points);
    let mut z_coords = Vec::with_capacity(out_points);
    let mut original_indices = Vec::with_capacity(out_points);
    let mut iblank_vec = grid.iblank.as_ref().map(|_| Vec::with_capacity(out_points));

    let ni_usize = ni as usize;
    let nj_usize = nj as usize;

    for k_idx in k0..=k1 {
        for j_idx in j0..=j1 {
            for i_idx in i0..=i1 {
                let orig = i_idx + j_idx * ni_usize + k_idx * ni_usize * nj_usize;
                x_coords.push(grid.x_coords[orig]);
                y_coords.push(grid.y_coords[orig]);
                z_coords.push(grid.z_coords[orig]);
                original_indices.push(orig);
                if let Some(ref mut ib) = iblank_vec {
                    ib.push(grid.iblank.as_ref().map(|src| src[orig]).unwrap_or(1));
                }
            }
        }
    }

    Ok((
        Plot3DGrid {
            dimensions: GridDimensions {
                i: out_i as u32,
                j: out_j as u32,
                k: out_k as u32,
            },
            x_coords,
            y_coords,
            z_coords,
            iblank: iblank_vec,
        },
        original_indices,
    ))
}

/// Extract a subset volume from a cached grid using 1-based inclusive ranges.
#[allow(non_snake_case)]
#[tauri::command]
fn subset_grid_by_id(
    gridId: String,
    iStart: Option<u32>,
    iEnd: Option<u32>,
    jStart: Option<u32>,
    jEnd: Option<u32>,
    kStart: Option<u32>,
    kEnd: Option<u32>,
) -> Result<Plot3DGrid, String> {
    let grid = {
        let cache = GRID_CACHE
            .lock()
            .map_err(|_| "Grid cache lock poisoned".to_string())?;
        let cached = cache
            .get(&gridId)
            .ok_or_else(|| format!("Grid not found in cache: {}", gridId))?;
        Arc::clone(&cached.grid)
    };

    let (subset, _) = build_subset_grid(&grid, iStart, iEnd, jStart, jEnd, kStart, kEnd)?;
    Ok(subset)
}

/// Slice a cached grid with arbitrary plane (ID-based)
#[allow(non_snake_case)]
#[tauri::command]
fn slice_arbitrary_plane_by_id(
    gridId: String,
    planePoint: [f32; 3],
    planeNormal: [f32; 3],
    respect_iblank: Option<bool>,
    show_fringe_points: Option<bool>,
    iblank_filter_mode: Option<String>,
    _window: WebviewWindow,
) -> Result<MeshGeometry, String> {
    let (effective_respect_iblank, effective_show_fringe_points, effective_filter_mode) =
        normalize_iblank_flags(respect_iblank, show_fringe_points, iblank_filter_mode);

    log_debug(&format!(
        "Slicing cached grid {} with arbitrary plane: point={:?}, normal={:?}",
        gridId, planePoint, planeNormal
    ));

    // Load grid from cache
    let grid = {
        let cache = GRID_CACHE
            .lock()
            .map_err(|_| "Grid cache lock poisoned".to_string())?;
        let cached = cache
            .get(&gridId)
            .ok_or_else(|| format!("Grid not found in cache: {}", gridId))?;
        Arc::clone(&cached.grid)
    };

    let result = grid.slice_arbitrary_plane(
        planePoint,
        planeNormal,
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
    );

    match &result {
        Ok(mesh) => {
            log_info(&format!(
                "Arbitrary plane slice generated: {} vertices, {} triangles",
                mesh.vertex_count,
                mesh.triangle_indices.len() / 3
            ));
        }
        Err(e) => {
            log_error(&format!("Failed to slice arbitrary plane: {}", e));
        }
    }

    result
}

/// Compute solution colors using cached grid and solution (ID-based)
#[allow(non_snake_case)]
#[tauri::command]
fn compute_solution_colors(
    gridId: String,
    solutionId: String,
    field: String,
    colorScheme: String,
    respect_iblank: Option<bool>,
    show_fringe_points: Option<bool>,
    iblank_filter_mode: Option<String>,
    window: WebviewWindow,
) -> Result<MeshGeometry, String> {
    use plot_state::ScalarField;
    use solution::{compute_colors, compute_scalar_field_surface_with_grid, ColorScheme};

    let _ = window.emit("loading-start", format!("Computing {} field...", field));

    let (effective_respect_iblank, effective_show_fringe_points, effective_filter_mode) =
        normalize_iblank_flags(respect_iblank, show_fringe_points, iblank_filter_mode);

    // Load grid from cache
    let (grid, grid_file_path, grid_index) = {
        let cache = GRID_CACHE
            .lock()
            .map_err(|_| "Grid cache lock poisoned".to_string())?;
        let cached = cache
            .get(&gridId)
            .ok_or_else(|| format!("Grid not found in cache: {}", gridId))?;
        (
            Arc::clone(&cached.grid),
            cached.file_path.clone(),
            cached.grid_index,
        )
    };

    // Load solution from cache
    let (solution, solution_file_path, solution_grid_index) = {
        let cache = SOLUTION_CACHE_V2
            .lock()
            .map_err(|_| "Solution cache lock poisoned".to_string())?;
        let cached = cache
            .get(&solutionId)
            .ok_or_else(|| format!("Solution not found in cache: {}", solutionId))?;
        (
            Arc::clone(&cached.solution),
            cached.file_path.clone(),
            cached.grid_index,
        )
    };

    if grid_index != solution_grid_index {
        return Err(format!(
            "Grid/solution mismatch: grid/solution index differs: grid(id={}, index={}) vs solution(id={}, index={})",
            gridId, grid_index, solutionId, solution_grid_index
        ));
    }

    if grid.dimensions.i != solution.dimensions.i
        || grid.dimensions.j != solution.dimensions.j
        || grid.dimensions.k != solution.dimensions.k
    {
        return Err(format!(
            "Grid/solution mismatch: dimensions differ: grid(id={}, dims={}x{}x{}) vs solution(id={}, dims={}x{}x{})",
            gridId,
            grid.dimensions.i,
            grid.dimensions.j,
            grid.dimensions.k,
            solutionId,
            solution.dimensions.i,
            solution.dimensions.j,
            solution.dimensions.k
        ));
    }

    if grid_file_path != solution_file_path {
        log_debug(&format!(
            "Grid/solution file paths differ but pair accepted by index+dimensions: grid(id={}, file={}) solution(id={}, file={})",
            gridId, grid_file_path, solutionId, solution_file_path
        ));
    }

    // Parse field and scheme
    let field_enum =
        ScalarField::from_str(&field).ok_or_else(|| format!("Unknown scalar field: {}", field))?;
    let scheme = ColorScheme::from_str(&colorScheme)
        .ok_or_else(|| format!("Unknown color scheme: {}", colorScheme))?;

    // Validate
    let grid_points = grid.total_points();
    if solution.rho.len() != grid_points {
        return Err(format!(
            "Solution points {} != grid points {}",
            solution.rho.len(),
            grid_points
        ));
    }

    // Auto-detect decimation
    let i = grid.dimensions.i as usize;
    let j = grid.dimensions.j as usize;
    let max_dim = i.max(j);

    let decimation_factor = if max_dim > 1000 {
        4
    } else if max_dim > 500 {
        3
    } else if max_dim > 250 {
        2
    } else {
        1
    };

    if decimation_factor > 1 {
        log_info(&format!(
            "Solution grid size {}x{} - applying {}x decimation for performance",
            i, j, decimation_factor
        ));
    }

    // Compute colors and scalar values (use grid for derivative fields)
    let values =
        compute_scalar_field_surface_with_grid(&solution, &grid, field_enum, decimation_factor);
    let probe_components = build_surface_probe_components(
        &solution,
        grid.dimensions.i as usize,
        grid.dimensions.j as usize,
        decimation_factor.max(1),
    );
    let probe_ijk = build_surface_probe_ijk(
        grid.dimensions.i as usize,
        grid.dimensions.j as usize,
        decimation_factor.max(1),
    );
    let colors = compute_colors(&values, &scheme);

    let mut mesh = grid.to_mesh_surface_geometry_decimated(
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
        decimation_factor,
    );
    mesh.colors = align_surface_mesh_colors(
        &mut mesh,
        &colors,
        grid.iblank.as_ref(),
        grid.dimensions.i as usize,
        grid.dimensions.j as usize,
        decimation_factor.max(1),
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
    );

    // Also populate scalar values for point probe
    mesh.scalar_values = align_surface_mesh_scalar_values(
        &mut mesh,
        &values,
        grid.iblank.as_ref(),
        grid.dimensions.i as usize,
        grid.dimensions.j as usize,
        decimation_factor.max(1),
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
    );
    mesh.probe_components = align_surface_mesh_probe_components(
        &mut mesh,
        &probe_components,
        grid.iblank.as_ref(),
        grid.dimensions.i as usize,
        grid.dimensions.j as usize,
        decimation_factor.max(1),
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
    );
    mesh.probe_ijk = align_surface_mesh_probe_ijk(
        &mut mesh,
        &probe_ijk,
        grid.iblank.as_ref(),
        grid.dimensions.i as usize,
        grid.dimensions.j as usize,
        decimation_factor.max(1),
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
    );

    let _ = window.emit("loading-end", ());

    Ok(mesh)
}

/// Compute solution colors for sliced grid using cached data (ID-based)
#[allow(non_snake_case)]
#[tauri::command]
fn compute_solution_colors_sliced(
    gridId: String,
    solutionId: String,
    slicePlane: String,
    sliceIndex: u32,
    field: String,
    colorScheme: String,
    respect_iblank: Option<bool>,
    show_fringe_points: Option<bool>,
    iblank_filter_mode: Option<String>,
    global_min: Option<f32>,
    global_max: Option<f32>,
    window: WebviewWindow,
) -> Result<MeshGeometry, String> {
    use plot_state::ScalarField;
    use solution::{compute_colors_with_range, compute_scalar_field_with_grid, ColorScheme};

    let _ = window.emit(
        "loading-start",
        format!("Computing {} field on slice...", field),
    );

    let (effective_respect_iblank, effective_show_fringe_points, effective_filter_mode) =
        normalize_iblank_flags(respect_iblank, show_fringe_points, iblank_filter_mode);

    // Load grid from cache
    let (original_grid, grid_file_path, grid_index) = {
        let cache = GRID_CACHE
            .lock()
            .map_err(|_| "Grid cache lock poisoned".to_string())?;
        let cached = cache
            .get(&gridId)
            .ok_or_else(|| format!("Grid not found in cache: {}", gridId))?;
        (
            Arc::clone(&cached.grid),
            cached.file_path.clone(),
            cached.grid_index,
        )
    };

    // Load solution from cache
    let (solution, solution_file_path, solution_grid_index) = {
        let cache = SOLUTION_CACHE_V2
            .lock()
            .map_err(|_| "Solution cache lock poisoned".to_string())?;
        let cached = cache
            .get(&solutionId)
            .ok_or_else(|| format!("Solution not found in cache: {}", solutionId))?;
        (
            Arc::clone(&cached.solution),
            cached.file_path.clone(),
            cached.grid_index,
        )
    };

    if grid_index != solution_grid_index {
        return Err(format!(
            "Grid/solution mismatch: grid/solution index differs: grid(id={}, index={}) vs solution(id={}, index={})",
            gridId, grid_index, solutionId, solution_grid_index
        ));
    }

    if original_grid.dimensions.i != solution.dimensions.i
        || original_grid.dimensions.j != solution.dimensions.j
        || original_grid.dimensions.k != solution.dimensions.k
    {
        return Err(format!(
            "Grid/solution mismatch: dimensions differ: grid(id={}, dims={}x{}x{}) vs solution(id={}, dims={}x{}x{})",
            gridId,
            original_grid.dimensions.i,
            original_grid.dimensions.j,
            original_grid.dimensions.k,
            solutionId,
            solution.dimensions.i,
            solution.dimensions.j,
            solution.dimensions.k
        ));
    }

    if grid_file_path != solution_file_path {
        log_debug(&format!(
            "Grid/solution file paths differ but pair accepted by index+dimensions: grid(id={}, file={}) solution(id={}, file={})",
            gridId, grid_file_path, solutionId, solution_file_path
        ));
    }

    // Parse field and scheme
    let field_enum =
        ScalarField::from_str(&field).ok_or_else(|| format!("Unknown scalar field: {}", field))?;
    let scheme = ColorScheme::from_str(&colorScheme)
        .ok_or_else(|| format!("Unknown color scheme: {}", colorScheme))?;

    // Perform slice
    let sliced_grid = original_grid
        .slice_grid(&slicePlane, sliceIndex)
        .map_err(|e| format!("Failed to slice grid: {}", e))?;

    // Validate solution matches original grid
    let grid_points = original_grid.total_points();
    if solution.rho.len() != grid_points {
        return Err(format!(
            "Solution points {} != grid points {}",
            solution.rho.len(),
            grid_points
        ));
    }

    // Extract dimensions
    let i_orig = original_grid.dimensions.i as usize;
    let j_orig = original_grid.dimensions.j as usize;
    let _k_orig = original_grid.dimensions.k as usize;

    let i_slice = sliced_grid.dimensions.i as usize;
    let j_slice = sliced_grid.dimensions.j as usize;

    let slice_idx = sliceIndex as usize;

    // Map each point in sliced grid to original grid for solution values
    // Pre-compute full scalar field (including derivative fields via grid).
    let full_field = compute_scalar_field_with_grid(&solution, &original_grid, field_enum);

    let mut values = Vec::with_capacity(sliced_grid.total_points());
    let mut probe_components =
        Vec::with_capacity(sliced_grid.total_points() * PROBE_COMPONENT_STRIDE);
    let mut probe_ijk = Vec::with_capacity(sliced_grid.total_points() * PROBE_IJK_STRIDE);

    let linear_index_original =
        |i: usize, j: usize, k: usize| -> usize { i + j * i_orig + k * i_orig * j_orig };

    match slicePlane.to_uppercase().as_str() {
        "K" => {
            for j_idx in 0..j_slice {
                for i_idx in 0..i_slice {
                    let orig_linear = linear_index_original(i_idx, j_idx, slice_idx);
                    values.push(full_field[orig_linear]);
                    push_probe_components_at(&solution, orig_linear, &mut probe_components);
                    push_probe_ijk(i_idx, j_idx, slice_idx, &mut probe_ijk);
                }
            }
        }
        "J" => {
            for k_idx in 0..j_slice {
                for i_idx in 0..i_slice {
                    let orig_linear = linear_index_original(i_idx, slice_idx, k_idx);
                    values.push(full_field[orig_linear]);
                    push_probe_components_at(&solution, orig_linear, &mut probe_components);
                    push_probe_ijk(i_idx, slice_idx, k_idx, &mut probe_ijk);
                }
            }
        }
        "I" => {
            for k_idx in 0..j_slice {
                for j_idx in 0..i_slice {
                    let orig_linear = linear_index_original(slice_idx, j_idx, k_idx);
                    values.push(full_field[orig_linear]);
                    push_probe_components_at(&solution, orig_linear, &mut probe_components);
                    push_probe_ijk(slice_idx, j_idx, k_idx, &mut probe_ijk);
                }
            }
        }
        _ => {
            return Err(format!("Invalid slice plane: {}", slicePlane));
        }
    }

    // Log displayed range (values on this shown slice) and normalization range
    let mut shown_min: Option<f32> = None;
    let mut shown_max: Option<f32> = None;
    for &v in values.iter() {
        if !v.is_finite() {
            continue;
        }
        shown_min = Some(match shown_min {
            Some(current) => current.min(v),
            None => v,
        });
        shown_max = Some(match shown_max {
            Some(current) => current.max(v),
            None => v,
        });
    }

    match (shown_min, shown_max) {
        (Some(smin), Some(smax)) => {
            let (nmin, nmax, source) = match (global_min, global_max) {
                (Some(gmin), Some(gmax)) => (gmin, gmax, "global_solution"),
                _ => (smin, smax, "shown_slice"),
            };
            log_debug(&format!(
                "[color-range][slice] gridId={} solutionId={} field={} plane={} idx={} shown=[{:.6e}, {:.6e}] normalize=[{:.6e}, {:.6e}] source={} count={}",
                gridId,
                solutionId,
                field,
                slicePlane,
                sliceIndex,
                smin,
                smax,
                nmin,
                nmax,
                source,
                values.len()
            ));
        }
        _ => {
            log_debug(&format!(
                "[color-range][slice] gridId={} solutionId={} field={} plane={} idx={} no finite values (count={})",
                gridId,
                solutionId,
                field,
                slicePlane,
                sliceIndex,
                values.len()
            ));
        }
    }

    let colors = compute_colors_with_range(&values, &scheme, global_min, global_max);

    let mut mesh = sliced_grid.to_mesh_surface_geometry_decimated(
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
        1,
    );
    mesh.colors = align_surface_mesh_colors(
        &mut mesh,
        &colors,
        sliced_grid.iblank.as_ref(),
        sliced_grid.dimensions.i as usize,
        sliced_grid.dimensions.j as usize,
        1,
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
    );

    // Also populate scalar values for point probe
    mesh.scalar_values = align_surface_mesh_scalar_values(
        &mut mesh,
        &values,
        sliced_grid.iblank.as_ref(),
        sliced_grid.dimensions.i as usize,
        sliced_grid.dimensions.j as usize,
        1,
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
    );
    mesh.probe_components = align_surface_mesh_probe_components(
        &mut mesh,
        &probe_components,
        sliced_grid.iblank.as_ref(),
        sliced_grid.dimensions.i as usize,
        sliced_grid.dimensions.j as usize,
        1,
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
    );
    mesh.probe_ijk = align_surface_mesh_probe_ijk(
        &mut mesh,
        &probe_ijk,
        sliced_grid.iblank.as_ref(),
        sliced_grid.dimensions.i as usize,
        sliced_grid.dimensions.j as usize,
        1,
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
    );

    let _ = window.emit("loading-end", ());

    Ok(mesh)
}

/// Compute solution colors on a subset volume from cached grid/solution data.
#[allow(non_snake_case)]
#[tauri::command]
fn compute_solution_colors_subset_by_id(
    gridId: String,
    solutionId: String,
    iStart: Option<u32>,
    iEnd: Option<u32>,
    jStart: Option<u32>,
    jEnd: Option<u32>,
    kStart: Option<u32>,
    kEnd: Option<u32>,
    field: String,
    colorScheme: String,
    respect_iblank: Option<bool>,
    show_fringe_points: Option<bool>,
    iblank_filter_mode: Option<String>,
    global_min: Option<f32>,
    global_max: Option<f32>,
    window: WebviewWindow,
) -> Result<MeshGeometry, String> {
    use plot_state::ScalarField;
    use solution::{compute_colors_with_range, compute_scalar_field_with_grid, ColorScheme};

    let _ = window.emit(
        "loading-start",
        format!("Computing {} field on subset...", field),
    );

    let (effective_respect_iblank, effective_show_fringe_points, effective_filter_mode) =
        normalize_iblank_flags(respect_iblank, show_fringe_points, iblank_filter_mode);

    let (grid, grid_file_path, grid_index) = {
        let cache = GRID_CACHE
            .lock()
            .map_err(|_| "Grid cache lock poisoned".to_string())?;
        let cached = cache
            .get(&gridId)
            .ok_or_else(|| format!("Grid not found in cache: {}", gridId))?;
        (
            Arc::clone(&cached.grid),
            cached.file_path.clone(),
            cached.grid_index,
        )
    };

    let (solution, solution_file_path, solution_grid_index) = {
        let cache = SOLUTION_CACHE_V2
            .lock()
            .map_err(|_| "Solution cache lock poisoned".to_string())?;
        let cached = cache
            .get(&solutionId)
            .ok_or_else(|| format!("Solution not found in cache: {}", solutionId))?;
        (
            Arc::clone(&cached.solution),
            cached.file_path.clone(),
            cached.grid_index,
        )
    };

    if grid_index != solution_grid_index {
        return Err(format!(
            "Grid/solution mismatch: grid(id={}, index={}) vs solution(id={}, index={})",
            gridId, grid_index, solutionId, solution_grid_index
        ));
    }

    if grid.dimensions.i != solution.dimensions.i
        || grid.dimensions.j != solution.dimensions.j
        || grid.dimensions.k != solution.dimensions.k
    {
        return Err(format!(
            "Grid/solution mismatch: dimensions differ: grid(id={}, dims={}x{}x{}) vs solution(id={}, dims={}x{}x{})",
            gridId,
            grid.dimensions.i,
            grid.dimensions.j,
            grid.dimensions.k,
            solutionId,
            solution.dimensions.i,
            solution.dimensions.j,
            solution.dimensions.k
        ));
    }

    if grid_file_path != solution_file_path {
        log_debug(&format!(
            "Grid/solution file paths differ but pair accepted by index+dimensions: grid(id={}, file={}) solution(id={}, file={})",
            gridId, grid_file_path, solutionId, solution_file_path
        ));
    }

    let field_enum =
        ScalarField::from_str(&field).ok_or_else(|| format!("Unknown scalar field: {}", field))?;
    let scheme = ColorScheme::from_str(&colorScheme)
        .ok_or_else(|| format!("Unknown color scheme: {}", colorScheme))?;

    if solution.rho.len() != grid.total_points() {
        return Err(format!(
            "Solution points {} != grid points {}",
            solution.rho.len(),
            grid.total_points()
        ));
    }

    let (subset_grid, original_indices) =
        build_subset_grid(&grid, iStart, iEnd, jStart, jEnd, kStart, kEnd)?;

    // Pre-compute full scalar field (including derivative fields via grid).
    let full_field = compute_scalar_field_with_grid(&solution, &grid, field_enum);

    let mut values = Vec::with_capacity(original_indices.len());
    let mut probe_components = Vec::with_capacity(original_indices.len() * PROBE_COMPONENT_STRIDE);
    let mut probe_ijk = Vec::with_capacity(original_indices.len() * PROBE_IJK_STRIDE);
    for &orig in &original_indices {
        values.push(full_field[orig]);
        push_probe_components_at(&solution, orig, &mut probe_components);
        let (i_idx, j_idx, k_idx) =
            linear_index_to_ijk(orig, grid.dimensions.i as usize, grid.dimensions.j as usize);
        push_probe_ijk(i_idx, j_idx, k_idx, &mut probe_ijk);
    }

    let colors = compute_colors_with_range(&values, &scheme, global_min, global_max);
    let mut mesh = subset_grid.to_mesh_surface_geometry_decimated(
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
        1,
    );

    // Determine the 2D surface dimensions that to_mesh_surface_geometry_decimated uses.
    // The orientation logic mirrors what that function does: K-plane when nk==1 is absent,
    // I-plane when ni==1, J-plane when nj==1, otherwise K-min boundary (ni, nj).
    let ni_sub = subset_grid.dimensions.i as usize;
    let nj_sub = subset_grid.dimensions.j as usize;
    let nk_sub = subset_grid.dimensions.k as usize;
    let (surf_u, surf_v) = if nk_sub == 1 {
        (ni_sub, nj_sub)
    } else if ni_sub == 1 {
        (nj_sub, nk_sub)
    } else if nj_sub == 1 {
        (ni_sub, nk_sub)
    } else {
        (ni_sub, nj_sub)
    };

    mesh.colors = align_surface_mesh_colors(
        &mut mesh,
        &colors,
        subset_grid.iblank.as_ref(),
        surf_u,
        surf_v,
        1,
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
    );

    mesh.scalar_values = align_surface_mesh_scalar_values(
        &mut mesh,
        &values,
        subset_grid.iblank.as_ref(),
        surf_u,
        surf_v,
        1,
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
    );

    mesh.probe_components = align_surface_mesh_probe_components(
        &mut mesh,
        &probe_components,
        subset_grid.iblank.as_ref(),
        surf_u,
        surf_v,
        1,
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
    );
    mesh.probe_ijk = align_surface_mesh_probe_ijk(
        &mut mesh,
        &probe_ijk,
        subset_grid.iblank.as_ref(),
        surf_u,
        surf_v,
        1,
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
    );

    let _ = window.emit("loading-end", ());
    Ok(mesh)
}

/// Compute solution colors for arbitrary plane using cached data (ID-based)
#[allow(non_snake_case)]
#[tauri::command]
fn compute_solution_colors_arbitrary_plane(
    gridId: String,
    solutionId: String,
    planePoint: [f32; 3],
    planeNormal: [f32; 3],
    field: String,
    colorScheme: String,
    respect_iblank: Option<bool>,
    show_fringe_points: Option<bool>,
    iblank_filter_mode: Option<String>,
    global_min: Option<f32>,
    global_max: Option<f32>,
    window: WebviewWindow,
) -> Result<MeshGeometry, String> {
    use plot_state::ScalarField;
    use solution::{compute_colors_with_range, ColorScheme};

    struct LoadingEndGuard {
        window: WebviewWindow,
    }

    impl Drop for LoadingEndGuard {
        fn drop(&mut self) {
            let _ = self.window.emit("loading-end", ());
        }
    }

    let _ = window.emit(
        "loading-start",
        format!("Computing {} field on arbitrary plane...", field),
    );
    let _loading_end_guard = LoadingEndGuard {
        window: window.clone(),
    };

    let (effective_respect_iblank, effective_show_fringe_points, effective_filter_mode) =
        normalize_iblank_flags(respect_iblank, show_fringe_points, iblank_filter_mode);

    // Load grid from cache
    let (grid, grid_file_path, grid_index) = {
        let cache = GRID_CACHE
            .lock()
            .map_err(|_| "Grid cache lock poisoned".to_string())?;
        let cached = cache
            .get(&gridId)
            .ok_or_else(|| format!("Grid not found in cache: {}", gridId))?;
        (
            Arc::clone(&cached.grid),
            cached.file_path.clone(),
            cached.grid_index,
        )
    };

    // Load solution from cache
    let (solution, solution_file_path, solution_grid_index) = {
        let cache = SOLUTION_CACHE_V2
            .lock()
            .map_err(|_| "Solution cache lock poisoned".to_string())?;
        let cached = cache
            .get(&solutionId)
            .ok_or_else(|| format!("Solution not found in cache: {}", solutionId))?;
        (
            Arc::clone(&cached.solution),
            cached.file_path.clone(),
            cached.grid_index,
        )
    };

    if grid_index != solution_grid_index {
        return Err(format!(
            "Grid/solution mismatch: grid/solution index differs: grid(id={}, index={}) vs solution(id={}, index={})",
            gridId, grid_index, solutionId, solution_grid_index
        ));
    }
    if grid.dimensions.i != solution.dimensions.i
        || grid.dimensions.j != solution.dimensions.j
        || grid.dimensions.k != solution.dimensions.k
    {
        return Err(format!(
            "Grid/solution mismatch: dimensions differ: grid(id={}, dims={}x{}x{}) vs solution(id={}, dims={}x{}x{})",
            gridId,
            grid.dimensions.i,
            grid.dimensions.j,
            grid.dimensions.k,
            solutionId,
            solution.dimensions.i,
            solution.dimensions.j,
            solution.dimensions.k
        ));
    }

    if grid_file_path != solution_file_path {
        log_debug(&format!(
            "Grid/solution file paths differ but pair accepted by index+dimensions: grid(id={}, file={}) solution(id={}, file={})",
            gridId, grid_file_path, solutionId, solution_file_path
        ));
    }

    // Parse field and scheme
    let field_enum =
        ScalarField::from_str(&field).ok_or_else(|| format!("Unknown scalar field: {}", field))?;
    let scheme = ColorScheme::from_str(&colorScheme)
        .ok_or_else(|| format!("Unknown color scheme: {}", colorScheme))?;

    // Validate
    let grid_points = grid.total_points();
    if solution.rho.len() != grid_points {
        return Err(format!(
            "Solution points {} != grid points {}",
            solution.rho.len(),
            grid_points
        ));
    }

    // Slice with solution tracking
    let mut mesh = grid.slice_arbitrary_plane_with_solution(
        planePoint,
        planeNormal,
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
    )?;

    let vertex_cell_data = mesh
        .vertex_cell_data
        .as_ref()
        .ok_or_else(|| "No vertex cell data available".to_string())?;

    // Precompute scalar field at original grid nodes (including derivative fields via grid),
    // then interpolate to arbitrary-plane vertices using the stored corner weights.
    use solution::compute_scalar_field_with_grid;
    let nodal_field_values: Vec<f32> = compute_scalar_field_with_grid(&solution, &grid, field_enum);

    let i_orig = grid.dimensions.i as usize;
    let j_orig = grid.dimensions.j as usize;
    let linear_index =
        |i: usize, j: usize, k: usize| -> usize { i + j * i_orig + k * i_orig * j_orig };

    let mut values = Vec::with_capacity(vertex_cell_data.len());
    let mut probe_components = Vec::with_capacity(vertex_cell_data.len() * PROBE_COMPONENT_STRIDE);
    let mut probe_ijk = Vec::with_capacity(vertex_cell_data.len() * PROBE_IJK_STRIDE);

    let nodal_gamma_values: Vec<f32> = (0..grid_points)
        .map(|idx| {
            solution
                .gamma
                .as_ref()
                .and_then(|g| g.get(idx))
                .copied()
                .unwrap_or(1.4)
        })
        .collect();

    for cell_data in vertex_cell_data {
        let i = cell_data.cell_i;
        let j = cell_data.cell_j;
        let k = cell_data.cell_k;

        let corner_indices = [
            linear_index(i, j, k),
            linear_index(i + 1, j, k),
            linear_index(i + 1, j + 1, k),
            linear_index(i, j + 1, k),
            linear_index(i, j, k + 1),
            linear_index(i + 1, j, k + 1),
            linear_index(i + 1, j + 1, k + 1),
            linear_index(i, j + 1, k + 1),
        ];
        let corner_ijk = [
            (i, j, k),
            (i + 1, j, k),
            (i + 1, j + 1, k),
            (i, j + 1, k),
            (i, j, k + 1),
            (i + 1, j, k + 1),
            (i + 1, j + 1, k + 1),
            (i, j + 1, k + 1),
        ];

        let mut interpolated_field = 0.0;

        for (idx, &corner_idx) in corner_indices.iter().enumerate() {
            let weight = cell_data.weights[idx];
            interpolated_field += weight * nodal_field_values[corner_idx];
        }

        values.push(interpolated_field);

        let mut interpolated_rho = 0.0;
        let mut interpolated_rhou = 0.0;
        let mut interpolated_rhov = 0.0;
        let mut interpolated_rhow = 0.0;
        let mut interpolated_rhoe = 0.0;
        let mut interpolated_gamma = 0.0;

        for (idx, &corner_idx) in corner_indices.iter().enumerate() {
            let weight = cell_data.weights[idx];
            interpolated_rho += weight * solution.rho[corner_idx];
            interpolated_rhou += weight * solution.rhou[corner_idx];
            interpolated_rhov += weight * solution.rhov[corner_idx];
            interpolated_rhow += weight * solution.rhow[corner_idx];
            interpolated_rhoe += weight * solution.rhoe[corner_idx];
            interpolated_gamma += weight * nodal_gamma_values[corner_idx];
        }

        probe_components.extend_from_slice(&[
            interpolated_rho,
            interpolated_rhou,
            interpolated_rhov,
            interpolated_rhow,
            interpolated_rhoe,
            interpolated_gamma,
        ]);

        let mut max_weight_idx = 0usize;
        let mut max_weight = cell_data.weights[0];
        for idx in 1..cell_data.weights.len() {
            let weight = cell_data.weights[idx];
            if weight > max_weight {
                max_weight = weight;
                max_weight_idx = idx;
            }
        }
        let (ii, jj, kk) = corner_ijk[max_weight_idx];
        push_probe_ijk(ii, jj, kk, &mut probe_ijk);
    }

    // Log displayed range (values on this shown arbitrary plane) and normalization range
    let mut shown_min: Option<f32> = None;
    let mut shown_max: Option<f32> = None;
    for &v in values.iter() {
        if !v.is_finite() {
            continue;
        }
        shown_min = Some(match shown_min {
            Some(current) => current.min(v),
            None => v,
        });
        shown_max = Some(match shown_max {
            Some(current) => current.max(v),
            None => v,
        });
    }

    match (shown_min, shown_max) {
        (Some(smin), Some(smax)) => {
            let (nmin, nmax, source) = match (global_min, global_max) {
                (Some(gmin), Some(gmax)) => (gmin, gmax, "global_solution"),
                _ => (smin, smax, "shown_plane"),
            };
            log_debug(&format!(
                "[color-range][arbitrary] gridId={} solutionId={} field={} shown=[{:.6e}, {:.6e}] normalize=[{:.6e}, {:.6e}] source={} count={} planePoint={:?} planeNormal={:?}",
                gridId,
                solutionId,
                field,
                smin,
                smax,
                nmin,
                nmax,
                source,
                values.len(),
                planePoint,
                planeNormal
            ));
        }
        _ => {
            log_debug(&format!(
                "[color-range][arbitrary] gridId={} solutionId={} field={} no finite values (count={}) planePoint={:?} planeNormal={:?}",
                gridId,
                solutionId,
                field,
                values.len(),
                planePoint,
                planeNormal
            ));
        }
    }

    let colors = compute_colors_with_range(&values, &scheme, global_min, global_max);
    mesh.colors = Some(colors);

    // Also populate scalar values for point probe (same as values array)
    mesh.scalar_values = Some(values);
    mesh.probe_components = Some(probe_components);
    mesh.probe_ijk = Some(probe_ijk);

    Ok(mesh)
}

// ============================================================================
// Field range computation for global color normalization
// ============================================================================

#[derive(serde::Serialize)]
pub struct FieldRange {
    pub min: f32,
    pub max: f32,
}

fn is_derivative_scalar_field(field: plot_state::ScalarField) -> bool {
    use plot_state::ScalarField;
    matches!(
        field,
        ScalarField::Normalized2dStreamFunction
            | ScalarField::VelocityDivergence
            | ScalarField::VorticityX
            | ScalarField::VorticityY
            | ScalarField::VorticityZ
            | ScalarField::VorticityMagnitude
            | ScalarField::Swirl
            | ScalarField::VelocityCrossVorticityMagnitude
            | ScalarField::HelicityDensity
            | ScalarField::RelativeHelicity
            | ScalarField::FilteredRelativeHelicity
            | ScalarField::ShockFunctionPressureGradient
            | ScalarField::FilteredShockFunction
            | ScalarField::PressureGradientMagnitude
            | ScalarField::DensityGradientMagnitude
    )
}

fn compute_field_values_for_solution_id(
    solution_id: &str,
    field: plot_state::ScalarField,
) -> Result<Vec<f32>, String> {
    use solution::{compute_scalar_field, compute_scalar_field_with_grid};

    let (solution, solution_grid_index) = {
        let cache = SOLUTION_CACHE_V2
            .lock()
            .map_err(|_| "Solution cache lock poisoned".to_string())?;
        let cached = cache
            .get(solution_id)
            .ok_or_else(|| format!("Solution not found in cache: {}", solution_id))?;
        (Arc::clone(&cached.solution), cached.grid_index)
    };

    let preferred_grid_id = {
        let guard = PLOT_STATE
            .lock()
            .map_err(|_| "Plot state lock poisoned".to_string())?;
        guard.dataset.grid_id.clone()
    };

    let matching_grid = {
        let cache = GRID_CACHE
            .lock()
            .map_err(|_| "Grid cache lock poisoned".to_string())?;

        if let Some(grid_id) = preferred_grid_id.as_ref() {
            if let Some(cached) = cache.get(grid_id) {
                if cached.grid_index == solution_grid_index
                    && cached.grid.dimensions.i == solution.dimensions.i
                    && cached.grid.dimensions.j == solution.dimensions.j
                    && cached.grid.dimensions.k == solution.dimensions.k
                {
                    Some(Arc::clone(&cached.grid))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
        .or_else(|| {
            let mut candidate_ids: Vec<String> = cache
                .values()
                .filter(|cached| {
                    cached.grid_index == solution_grid_index
                        && cached.grid.dimensions.i == solution.dimensions.i
                        && cached.grid.dimensions.j == solution.dimensions.j
                        && cached.grid.dimensions.k == solution.dimensions.k
                })
                .map(|cached| cached.id.clone())
                .collect();
            candidate_ids.sort();
            candidate_ids
                .first()
                .and_then(|id| cache.get(id).map(|cached| Arc::clone(&cached.grid)))
        })
    };

    match matching_grid {
        Some(grid) => Ok(compute_scalar_field_with_grid(&solution, &grid, field)),
        None if is_derivative_scalar_field(field) => Err(format!(
            "Field {:?} requires matching grid coordinates, but no compatible grid is cached for solution {}",
            field, solution_id
        )),
        None => Ok(compute_scalar_field(&solution, field)),
    }
}

/// Compute the finite min/max of a scalar field without materializing the full
/// Vec<f32>.  For non-derivative fields the solution arrays are iterated
/// directly; derivative fields fall back to `compute_field_values_for_solution_id`
/// which does allocate because the spatial finite-difference pass is already
/// fully vectorized.
fn compute_scalar_field_range_streaming(
    solution_id: &str,
    field: plot_state::ScalarField,
) -> Result<(Option<f32>, Option<f32>, usize), String> {
    use plot_state::ScalarField;
    use solution::compute_scalar_field;

    // Derivative fields still require full materialization.
    if is_derivative_scalar_field(field) {
        let values = compute_field_values_for_solution_id(solution_id, field)?;
        let n = values.len();
        let (lo, hi) = values.iter().copied().filter(|v| v.is_finite()).fold(
            (None::<f32>, None::<f32>),
            |(lo, hi), v| {
                (
                    Some(lo.map_or(v, |m: f32| m.min(v))),
                    Some(hi.map_or(v, |m: f32| m.max(v))),
                )
            },
        );
        return Ok((lo, hi, n));
    }

    // For non-derivative fields, iterate the raw solution arrays to avoid a
    // full clone / allocation for the common case.
    let solution = {
        let cache = SOLUTION_CACHE_V2
            .lock()
            .map_err(|_| "Solution cache lock poisoned".to_string())?;
        let cached = cache
            .get(solution_id)
            .ok_or_else(|| format!("Solution not found in cache: {}", solution_id))?;
        Arc::clone(&cached.solution)
    };

    // Fast path for raw Q-file arrays — no allocation needed.
    let raw_iter: Option<Box<dyn Iterator<Item = f32> + '_>> = match field {
        ScalarField::Density => Some(Box::new(solution.rho.iter().copied())),
        ScalarField::MomentumX => Some(Box::new(solution.rhou.iter().copied())),
        ScalarField::MomentumY => Some(Box::new(solution.rhov.iter().copied())),
        ScalarField::MomentumZ => Some(Box::new(solution.rhow.iter().copied())),
        ScalarField::Energy => Some(Box::new(solution.rhoe.iter().copied())),
        _ => None,
    };

    if let Some(iter) = raw_iter {
        let n = solution.rho.len();
        let (lo, hi) =
            iter.filter(|v| v.is_finite())
                .fold((None::<f32>, None::<f32>), |(lo, hi), v| {
                    (
                        Some(lo.map_or(v, |m: f32| m.min(v))),
                        Some(hi.map_or(v, |m: f32| m.max(v))),
                    )
                });
        return Ok((lo, hi, n));
    }

    // All other non-derivative fields: compute and stream without an extra clone.
    let values = compute_scalar_field(&solution, field);
    let n = values.len();
    let (lo, hi) = values.into_iter().filter(|v| v.is_finite()).fold(
        (None::<f32>, None::<f32>),
        |(lo, hi), v| {
            (
                Some(lo.map_or(v, |m: f32| m.min(v))),
                Some(hi.map_or(v, |m: f32| m.max(v))),
            )
        },
    );
    Ok((lo, hi, n))
}

/// Get the min/max range of a scalar field from a cached solution
#[allow(non_snake_case)]
#[tauri::command]
fn get_solution_field_range(solutionId: String, field: String) -> Result<FieldRange, String> {
    use plot_state::ScalarField;

    // Parse field
    let field_enum =
        ScalarField::from_str(&field).ok_or_else(|| format!("Unknown scalar field: {}", field))?;

    let (min, max, grid_points) = compute_scalar_field_range_streaming(&solutionId, field_enum)?;

    match (min, max) {
        (Some(min), Some(max)) => {
            log_debug(&format!(
                "[color-range][solution] solutionId={} field={} range=[{:.6e}, {:.6e}] count={}",
                solutionId, field, min, max, grid_points
            ));
            Ok(FieldRange { min, max })
        }
        _ => {
            // No finite values - use default range
            log_debug(&format!(
                "[color-range][solution] solutionId={} field={} no finite values, default range=[0.0, 1.0] count={}",
                solutionId,
                field,
                grid_points
            ));
            Ok(FieldRange { min: 0.0, max: 1.0 })
        }
    }
}

// ============================================================================
// Contour Level Resolution
// ============================================================================

/// Result returned by the `resolve_contour_levels` command.
#[derive(serde::Serialize)]
pub struct ContourLevelsResult {
    pub levels: Vec<f64>,
    pub diagnostics: Vec<plot_state::Diagnostic>,
    /// Global scalar field minimum used to resolve the spec.
    pub field_min: f64,
    /// Global scalar field maximum used to resolve the spec.
    pub field_max: f64,
}

/// Resolve the current contour specification to an ordered list of absolute
/// physical field values for the given solution and scalar field.
///
/// This is the canonical resolution path.  The frontend must call this command
/// rather than implementing its own normalization / de-normalization logic.
#[allow(non_snake_case)]
#[tauri::command]
fn resolve_contour_levels(
    solutionId: String,
    scalarField: String,
) -> Result<ContourLevelsResult, String> {
    use plot_state::ScalarField;

    // Read current contour spec from shared state.
    let spec = {
        let guard = PLOT_STATE
            .lock()
            .map_err(|_| "Plot state lock poisoned".to_string())?;
        guard.contour_spec.clone()
    };

    // Early-exit for the trivial case — no solution range lookup required.
    if spec == plot_state::ContourSpec::None {
        return Ok(ContourLevelsResult {
            levels: vec![],
            diagnostics: vec![],
            field_min: 0.0,
            field_max: 0.0,
        });
    }

    // Parse scalar field.
    let field_enum = ScalarField::from_str(&scalarField)
        .ok_or_else(|| format!("Unknown scalar field: {}", scalarField))?;

    let field_values = compute_field_values_for_solution_id(&solutionId, field_enum)?;
    let mut min_val: Option<f32> = None;
    let mut max_val: Option<f32> = None;
    for value in field_values {
        if !value.is_finite() {
            continue;
        }
        min_val = Some(match min_val {
            Some(cur) => cur.min(value),
            None => value,
        });
        max_val = Some(match max_val {
            Some(cur) => cur.max(value),
            None => value,
        });
    }
    let (min, max) = match (min_val, max_val) {
        (Some(mn), Some(mx)) => (mn as f64, mx as f64),
        // Uniform / empty field — use a trivial range so resolve() sees min==max
        // and emits the appropriate diagnostic.
        _ => (0.0_f64, 0.0_f64),
    };

    let (levels, diagnostics) = spec.resolve(min, max);
    Ok(ContourLevelsResult {
        levels,
        diagnostics,
        field_min: min,
        field_max: max,
    })
}

// ============================================================================
// Contour Extraction Commands
// ============================================================================

/// Extract iso-surface from volume data at specified scalar field level
#[allow(non_snake_case)]
#[tauri::command]
fn extract_iso_surface_by_id(
    gridId: String,
    solutionId: String,
    scalarField: String,
    levelAbsolute: f64,
    respectIblank: Option<bool>,
    showFringePoints: Option<bool>,
    iblankFilterMode: Option<String>,
    _window: WebviewWindow,
) -> Result<MeshGeometry, String> {
    use plot_state::ScalarField;

    let (effective_respect_iblank, effective_show_fringe_points, effective_filter_mode) =
        normalize_iblank_flags(respectIblank, showFringePoints, iblankFilterMode);

    // Parse scalar field
    let field_enum = ScalarField::from_str(&scalarField)
        .ok_or_else(|| format!("Unknown scalar field: {}", scalarField))?;

    // Load grid from cache
    let grid = {
        let cache = GRID_CACHE
            .lock()
            .map_err(|_| "Grid cache lock poisoned".to_string())?;
        let cached = cache
            .get(&gridId)
            .ok_or_else(|| format!("Grid not found in cache: {}", gridId))?;
        Arc::clone(&cached.grid)
    };

    // Load solution from cache
    let solution = {
        let cache = SOLUTION_CACHE_V2
            .lock()
            .map_err(|_| "Solution cache lock poisoned".to_string())?;
        let cached = cache
            .get(&solutionId)
            .ok_or_else(|| format!("Solution not found in cache: {}", solutionId))?;
        Arc::clone(&cached.solution)
    };

    let level = levelAbsolute as f32;

    log_info(&format!(
        "Extracting iso-surface for grid {} at absolute level {}",
        gridId, level
    ));

    // TODO: Call marching cubes implementation (Step 3)
    grid.extract_iso_surface(
        &solution,
        field_enum,
        level,
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
    )
}

/// Extract contour lines from I/J/K slice at specified level
#[allow(non_snake_case)]
#[tauri::command]
fn extract_slice_contours_by_id(
    gridId: String,
    solutionId: String,
    plane: String,
    index: usize,
    scalarField: String,
    levelAbsolute: f64,
    respectIblank: Option<bool>,
    showFringePoints: Option<bool>,
    iblankFilterMode: Option<String>,
    _window: WebviewWindow,
) -> Result<Vec<f32>, String> {
    use plot_state::ScalarField;

    let (effective_respect_iblank, effective_show_fringe_points, effective_filter_mode) =
        normalize_iblank_flags(respectIblank, showFringePoints, iblankFilterMode);

    // Parse scalar field
    let field_enum = ScalarField::from_str(&scalarField)
        .ok_or_else(|| format!("Unknown scalar field: {}", scalarField))?;

    // Load grid from cache
    let grid = {
        let cache = GRID_CACHE
            .lock()
            .map_err(|_| "Grid cache lock poisoned".to_string())?;
        let cached = cache
            .get(&gridId)
            .ok_or_else(|| format!("Grid not found in cache: {}", gridId))?;
        Arc::clone(&cached.grid)
    };

    // Load solution from cache
    let solution = {
        let cache = SOLUTION_CACHE_V2
            .lock()
            .map_err(|_| "Solution cache lock poisoned".to_string())?;
        let cached = cache
            .get(&solutionId)
            .ok_or_else(|| format!("Solution not found in cache: {}", solutionId))?;
        Arc::clone(&cached.solution)
    };

    let level = levelAbsolute as f32;

    log_info(&format!(
        "Extracting slice contours for grid {} plane {} index {} at level {}",
        gridId, plane, index, level
    ));

    // TODO: Call contour-line extraction implementation (Step 4)
    grid.extract_slice_contours(
        &solution,
        &plane,
        index,
        field_enum,
        level,
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
    )
}

/// Extract contour lines from arbitrary plane at specified level
#[allow(non_snake_case)]
#[tauri::command]
fn extract_arbitrary_plane_contours_by_id(
    gridId: String,
    solutionId: String,
    planePoint: [f32; 3],
    planeNormal: [f32; 3],
    scalarField: String,
    levelAbsolute: f64,
    respectIblank: Option<bool>,
    showFringePoints: Option<bool>,
    iblankFilterMode: Option<String>,
    _window: WebviewWindow,
) -> Result<Vec<f32>, String> {
    use plot_state::ScalarField;

    let (effective_respect_iblank, effective_show_fringe_points, effective_filter_mode) =
        normalize_iblank_flags(respectIblank, showFringePoints, iblankFilterMode);

    // Parse scalar field
    let field_enum = ScalarField::from_str(&scalarField)
        .ok_or_else(|| format!("Unknown scalar field: {}", scalarField))?;

    // Load grid from cache
    let grid = {
        let cache = GRID_CACHE
            .lock()
            .map_err(|_| "Grid cache lock poisoned".to_string())?;
        let cached = cache
            .get(&gridId)
            .ok_or_else(|| format!("Grid not found in cache: {}", gridId))?;
        Arc::clone(&cached.grid)
    };

    // Load solution from cache
    let solution = {
        let cache = SOLUTION_CACHE_V2
            .lock()
            .map_err(|_| "Solution cache lock poisoned".to_string())?;
        let cached = cache
            .get(&solutionId)
            .ok_or_else(|| format!("Solution not found in cache: {}", solutionId))?;
        Arc::clone(&cached.solution)
    };

    let level = levelAbsolute as f32;
    let cache_key = arbitrary_plane_field_cache_key(
        &gridId,
        &solutionId,
        planePoint,
        planeNormal,
        &scalarField,
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
    );

    log_info(&format!(
        "Extracting arbitrary plane contours for grid {} at level {}",
        gridId, level
    ));

    let sample = get_or_build_arbitrary_plane_field_sample(
        &grid,
        &solution,
        field_enum,
        planePoint,
        planeNormal,
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
        &cache_key,
    )?;

    if sample.scalar_values.is_empty() {
        return Ok(Vec::new());
    }

    extract_contour_lines_from_triangles(
        sample.vertices.as_slice(),
        sample.triangle_indices.as_slice(),
        sample.scalar_values.as_slice(),
        level,
    )
}

/// Extract contour lines from arbitrary plane for multiple levels in one pass.
#[allow(non_snake_case)]
#[tauri::command]
fn extract_arbitrary_plane_contours_multi_by_id(
    gridId: String,
    solutionId: String,
    planePoint: [f32; 3],
    planeNormal: [f32; 3],
    scalarField: String,
    levelsAbsolute: Vec<f64>,
    respectIblank: Option<bool>,
    showFringePoints: Option<bool>,
    iblankFilterMode: Option<String>,
    _window: WebviewWindow,
) -> Result<Vec<Vec<f32>>, String> {
    use plot_state::ScalarField;

    let (effective_respect_iblank, effective_show_fringe_points, effective_filter_mode) =
        normalize_iblank_flags(respectIblank, showFringePoints, iblankFilterMode);

    let field_enum = ScalarField::from_str(&scalarField)
        .ok_or_else(|| format!("Unknown scalar field: {}", scalarField))?;

    let grid = {
        let cache = GRID_CACHE
            .lock()
            .map_err(|_| "Grid cache lock poisoned".to_string())?;
        let cached = cache
            .get(&gridId)
            .ok_or_else(|| format!("Grid not found in cache: {}", gridId))?;
        Arc::clone(&cached.grid)
    };

    let solution = {
        let cache = SOLUTION_CACHE_V2
            .lock()
            .map_err(|_| "Solution cache lock poisoned".to_string())?;
        let cached = cache
            .get(&solutionId)
            .ok_or_else(|| format!("Solution not found in cache: {}", solutionId))?;
        Arc::clone(&cached.solution)
    };

    let cache_key = arbitrary_plane_field_cache_key(
        &gridId,
        &solutionId,
        planePoint,
        planeNormal,
        &scalarField,
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
    );

    let sample = get_or_build_arbitrary_plane_field_sample(
        &grid,
        &solution,
        field_enum,
        planePoint,
        planeNormal,
        effective_respect_iblank,
        effective_show_fringe_points,
        effective_filter_mode,
        &cache_key,
    )?;

    if sample.scalar_values.is_empty() {
        return Ok(vec![Vec::new(); levelsAbsolute.len()]);
    }

    let mut all = Vec::with_capacity(levelsAbsolute.len());
    for level in levelsAbsolute.iter() {
        all.push(extract_contour_lines_from_triangles(
            sample.vertices.as_slice(),
            sample.triangle_indices.as_slice(),
            sample.scalar_values.as_slice(),
            *level as f32,
        )?);
    }
    Ok(all)
}

// ============================================================================
// Cache Management Commands
// ============================================================================

/// List all cached grids with their metadata
#[tauri::command]
fn list_cached_grids() -> Result<Vec<GridMetadata>, String> {
    let cache = GRID_CACHE
        .lock()
        .map_err(|_| "Grid cache lock poisoned".to_string())?;

    let metadata: Vec<GridMetadata> = cache
        .values()
        .map(|cached| {
            let has_solution = SOLUTION_CACHE_V2
                .lock()
                .ok()
                .and_then(|sol_cache| {
                    sol_cache
                        .values()
                        .any(|s| {
                            s.file_path == cached.file_path && s.grid_index == cached.grid_index
                        })
                        .then_some(true)
                })
                .unwrap_or(false);

            GridMetadata {
                id: cached.id.clone(),
                file_path: cached.file_path.clone(),
                file_name: cached.file_name.clone(),
                grid_index: cached.grid_index,
                dimensions: cached.grid.dimensions.clone(),
                has_iblank: cached.has_iblank,
                has_solution,
            }
        })
        .collect();

    Ok(metadata)
}

/// List all cached solutions with their metadata
#[tauri::command]
fn list_cached_solutions() -> Result<Vec<SolutionMetadata>, String> {
    let cache = SOLUTION_CACHE_V2
        .lock()
        .map_err(|_| "Solution cache lock poisoned".to_string())?;

    let metadata: Vec<SolutionMetadata> = cache
        .values()
        .map(|cached| SolutionMetadata {
            id: cached.id.clone(),
            file_path: cached.file_path.clone(),
            file_name: cached.file_name.clone(),
            grid_index: cached.grid_index,
            dimensions: cached.solution.dimensions.clone(),
        })
        .collect();

    Ok(metadata)
}

/// Get metadata for a specific cached grid
#[tauri::command]
fn get_grid_metadata(grid_id: String) -> Result<GridMetadata, String> {
    let cache = GRID_CACHE
        .lock()
        .map_err(|_| "Grid cache lock poisoned".to_string())?;

    let cached = cache
        .get(&grid_id)
        .ok_or_else(|| format!("Grid not found in cache: {}", grid_id))?;

    let has_solution = SOLUTION_CACHE_V2
        .lock()
        .ok()
        .and_then(|sol_cache| {
            sol_cache
                .values()
                .any(|s| s.file_path == cached.file_path && s.grid_index == cached.grid_index)
                .then_some(true)
        })
        .unwrap_or(false);

    Ok(GridMetadata {
        id: cached.id.clone(),
        file_path: cached.file_path.clone(),
        file_name: cached.file_name.clone(),
        grid_index: cached.grid_index,
        dimensions: cached.grid.dimensions.clone(),
        has_iblank: cached.has_iblank,
        has_solution,
    })
}

/// Clear all cached grids
#[tauri::command]
fn clear_grid_cache() -> Result<(), String> {
    let mut cache = GRID_CACHE
        .lock()
        .map_err(|_| "Grid cache lock poisoned".to_string())?;

    let count = cache.len();
    cache.clear();
    if let Ok(mut arbitrary_cache) = ARBITRARY_PLANE_FIELD_CACHE.lock() {
        arbitrary_cache.clear();
    }
    log_info(&format!("Cleared {} grids from cache", count));
    Ok(())
}

/// Clear all cached solutions (v2 cache)
#[tauri::command]
fn clear_solution_cache_v2() -> Result<(), String> {
    let mut cache = SOLUTION_CACHE_V2
        .lock()
        .map_err(|_| "Solution cache lock poisoned".to_string())?;

    let count = cache.len();
    cache.clear();
    if let Ok(mut arbitrary_cache) = ARBITRARY_PLANE_FIELD_CACHE.lock() {
        arbitrary_cache.clear();
    }
    log_info(&format!("Cleared {} solutions from cache", count));
    Ok(())
}

/// Unload a specific grid from cache
#[tauri::command]
fn unload_grid(grid_id: String) -> Result<(), String> {
    let mut cache = GRID_CACHE
        .lock()
        .map_err(|_| "Grid cache lock poisoned".to_string())?;

    if cache.remove(&grid_id).is_some() {
        if let Ok(mut arbitrary_cache) = ARBITRARY_PLANE_FIELD_CACHE.lock() {
            arbitrary_cache.retain(|k, _| !k.starts_with(&format!("{}|", grid_id)));
        }
        log_info(&format!("Unloaded grid from cache: {}", grid_id));
        Ok(())
    } else {
        Err(format!("Grid not found in cache: {}", grid_id))
    }
}

/// Unload a specific solution from cache
#[tauri::command]
fn unload_solution(solution_id: String) -> Result<(), String> {
    let mut cache = SOLUTION_CACHE_V2
        .lock()
        .map_err(|_| "Solution cache lock poisoned".to_string())?;

    if cache.remove(&solution_id).is_some() {
        if let Ok(mut arbitrary_cache) = ARBITRARY_PLANE_FIELD_CACHE.lock() {
            arbitrary_cache.retain(|k, _| !k.contains(&format!("|{}|", solution_id)));
        }
        log_info(&format!("Unloaded solution from cache: {}", solution_id));
        Ok(())
    } else {
        Err(format!("Solution not found in cache: {}", solution_id))
    }
}

#[tauri::command]
async fn open_file_dialog(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let file_path = app.dialog().file().blocking_pick_file();

    Ok(file_path.map(|f| f.to_string()))
}

#[tauri::command]
async fn open_multiple_files_dialog(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let file_paths = app.dialog().file().blocking_pick_files();

    Ok(file_paths
        .map(|files| files.iter().map(|f| f.to_string()).collect())
        .unwrap_or_default())
}

/// Detect if file is ASCII or binary format
#[tauri::command]
fn detect_file_format(path: String) -> Result<String, String> {
    let p = Path::new(&path);

    match p.extension().and_then(|e| e.to_str()) {
        Some("q") | Some("f") => {
            // Try to detect by reading first few bytes
            std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|data| {
                    if data.len() < 4 {
                        return Ok("unknown".to_string());
                    }

                    // Check if file looks like ASCII (text)
                    let first_chars = &data[..data.len().min(100)];
                    if first_chars
                        .iter()
                        .all(|&b| b.is_ascii_graphic() || b.is_ascii_whitespace())
                    {
                        Ok("ascii".to_string())
                    } else {
                        Ok("binary".to_string())
                    }
                })
        }
        _ => Ok("unknown".to_string()),
    }
}

/// Get all log entries
#[tauri::command]
fn get_log_entries() -> Result<Vec<LogEntry>, String> {
    Ok(get_logs())
}

/// Clear all log entries
#[tauri::command]
fn clear_log_entries() -> Result<(), String> {
    clear_logs();
    Ok(())
}

/// Export logs to a file
#[tauri::command]
fn export_logs_to_file(path: String) -> Result<(), String> {
    export_logs(&path).map_err(|e| {
        let error_msg = format!("Failed to export logs: {}", e);
        log_error(&error_msg);
        error_msg
    })?;
    log_info(&format!("Logs exported to {}", path));
    Ok(())
}

/// Open save file dialog for log export
#[tauri::command]
async fn save_log_file_dialog(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let file_path = app
        .dialog()
        .file()
        .add_filter("Text Files", &["txt"])
        .add_filter("All Files", &["*"])
        .set_file_name("overview-logs.txt")
        .blocking_save_file();

    Ok(file_path.map(|f| f.to_string()))
}

/// Write text content to a file
#[tauri::command]
fn write_text_file(path: String, contents: String) -> Result<(), String> {
    use std::fs;
    use std::io::Write;

    let normalized_path = normalize_dialog_path(&path);

    let mut file = fs::File::create(&normalized_path)
        .map_err(|e| format!("Failed to create file {}: {}", normalized_path, e))?;

    file.write_all(contents.as_bytes())
        .map_err(|e| format!("Failed to write to file {}: {}", normalized_path, e))?;

    log_info(&format!(
        "Successfully wrote {} bytes to {}",
        contents.len(),
        normalized_path
    ));
    Ok(())
}

/// Open save file dialog for PNG export
#[tauri::command]
async fn save_png_file_dialog(
    app: tauri::AppHandle,
    default_name: Option<String>,
) -> Result<Option<String>, String> {
    let file_name = default_name.unwrap_or_else(|| "plot_output.png".to_string());

    let file_path = app
        .dialog()
        .file()
        .add_filter("PNG Images", &["png"])
        .add_filter("All Files", &["*"])
        .set_file_name(&file_name)
        .blocking_save_file();

    Ok(file_path.map(|f| f.to_string()))
}

/// Write binary PNG data to a file
#[tauri::command]
fn write_png_file(path: String, png_data: Vec<u8>) -> Result<String, String> {
    use std::fs;
    use std::io::Write;

    let normalized_path = normalize_dialog_path(&path);

    let mut file = fs::File::create(&normalized_path)
        .map_err(|e| format!("Failed to create file {}: {}", normalized_path, e))?;

    file.write_all(&png_data)
        .map_err(|e| format!("Failed to write to file {}: {}", normalized_path, e))?;

    file.flush()
        .map_err(|e| format!("Failed to flush file {}: {}", normalized_path, e))?;

    let metadata = fs::metadata(&normalized_path)
        .map_err(|e| format!("Failed to stat file {}: {}", normalized_path, e))?;
    if metadata.len() == 0 {
        return Err(format!(
            "PNG export produced empty file at {}",
            normalized_path
        ));
    }

    log_info(&format!(
        "Successfully wrote PNG file {} ({} bytes)",
        normalized_path,
        png_data.len()
    ));
    Ok(normalized_path)
}

fn normalize_dialog_path(path: &str) -> String {
    if let Some(without_scheme) = path.strip_prefix("file://") {
        // Dialog paths can be returned as file URLs; decode common URL escapes.
        let decoded = without_scheme
            .replace("%20", " ")
            .replace("%5B", "[")
            .replace("%5D", "]")
            .replace("%28", "(")
            .replace("%29", ")")
            .replace("%2C", ",")
            .replace("%23", "#")
            .replace("%25", "%");

        #[cfg(target_os = "windows")]
        {
            if decoded.starts_with('/') && decoded.chars().nth(2) == Some(':') {
                return decoded[1..].to_string();
            }
        }

        return decoded;
    }

    path.to_string()
}

/// Print frontend debug messages to the terminal
#[tauri::command]
fn frontend_log(message: String) {
    println!("[frontend] {}", message);
}

/// Open the About window
#[tauri::command]
async fn open_about_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("about") {
        let _ = window.set_focus();
        Ok(())
    } else {
        WebviewWindow::builder(&app, "about", tauri::WebviewUrl::App("/about.html".into()))
            .title("About overview")
            .inner_size(600.0, 700.0)
            .resizable(true)
            .minimizable(true)
            .maximizable(false)
            .build()
            .map_err(|e| format!("Failed to create About window: {}", e))?;
        Ok(())
    }
}

/// Open (or focus) the Point Probe window
#[tauri::command]
async fn open_probe_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("point-probe") {
        // If already open, keep it visible but do not steal focus.
        let _ = window.show();
        Ok(())
    } else {
        let main_window = app.get_webview_window("main");
        let mut probe_builder = WebviewWindow::builder(
            &app,
            "point-probe",
            tauri::WebviewUrl::App("/probe.html".into()),
        );

        // Prefer placing the probe window to the right of the main window so
        // it does not overlap the active viewport when there is space.
        if let Some(main) = &main_window {
            if let (Ok(pos), Ok(size)) = (main.outer_position(), main.outer_size()) {
                let target_x = pos.x as f64 + size.width as f64 + 24.0;
                let target_y = pos.y as f64 + 24.0;
                probe_builder = probe_builder.position(target_x, target_y);
            }
        }

        probe_builder
            .title("Point Probe")
            .inner_size(420.0, 560.0)
            .resizable(true)
            .minimizable(true)
            .maximizable(false)
            .focused(true)
            .build()
            .map_err(|e| format!("Failed to create Point Probe window: {}", e))?;
        Ok(())
    }
}

/// Close the Point Probe window if it exists
#[tauri::command]
fn close_probe_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("point-probe") {
        window
            .close()
            .map_err(|e| format!("Failed to close Point Probe window: {}", e))?;
    }
    Ok(())
}

/// Update Point Probe window HTML payload by evaluating JS in the probe window
#[tauri::command]
fn update_probe_window_html(app: tauri::AppHandle, html: String) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("point-probe") {
        let html_js = serde_json::to_string(&html)
            .map_err(|e| format!("Failed to serialize probe HTML payload: {}", e))?;
        let script = format!(
            "(function() {{ const root = document.getElementById('root'); if (root) {{ root.innerHTML = {}; }} }})();",
            html_js
        );

        window
            .eval(&script)
            .map_err(|e| format!("Failed to evaluate probe HTML update script: {}", e))?;
        return Ok(());
    }
    Err("Point Probe window is not open".to_string())
}

/// Return the current `PlotState` for dev inspection and parity verification.
/// This command is intentionally read-only; mutations go through
/// `apply_plot_action`.
#[tauri::command]
fn get_plot_state() -> Result<PlotState, String> {
    PLOT_STATE
        .lock()
        .map(|s| s.clone())
        .map_err(|e| format!("Failed to lock plot state: {e}"))
}

/// Apply a `PlotAction` to the shared `PlotState` and return the resulting
/// state together with any diagnostics produced by the transition.
///
/// The frontend should call this instead of mutating state directly so that
/// script execution and GUI interactions always share the same state path.
#[tauri::command]
fn apply_plot_action(action: PlotAction) -> Result<ApplyActionResult, String> {
    let mut guard = PLOT_STATE
        .lock()
        .map_err(|e| format!("Failed to lock plot state: {e}"))?;
    let current = guard.clone();
    let (new_state, diagnostics) = apply_action(current, action);
    *guard = new_state.clone();
    Ok(ApplyActionResult {
        state: new_state,
        diagnostics,
    })
}

/// Convenience command: set scalar field via a stable argument shape.
#[tauri::command]
fn set_plot_scalar_field(field: plot_state::ScalarField) -> Result<ApplyActionResult, String> {
    apply_plot_action(PlotAction::SetScalarField(field))
}

/// Convenience command: set plot family (contour vs function-surface) via a stable argument shape.
#[tauri::command]
fn set_plot_family(family: plot_state::PlotFamily) -> Result<ApplyActionResult, String> {
    apply_plot_action(PlotAction::SetPlotFamily(family))
}

/// Convenience command: set camera viewpoint via a stable argument shape.
#[tauri::command]
fn set_plot_viewpoint(vp: plot_state::ViewPoint) -> Result<ApplyActionResult, String> {
    apply_plot_action(PlotAction::SetViewpoint(vp))
}

/// Convenience command: set named camera axis view via a stable argument shape.
#[tauri::command]
fn set_plot_axis_view(view: plot_state::AxisView) -> Result<ApplyActionResult, String> {
    apply_plot_action(PlotAction::SetAxisView(view))
}

/// Resolve a single `IndexRange` field, converting negative 1-based-from-end
/// indices to explicit positive 1-based indices using `dim` (the axis size).
fn resolve_index_range(range: &plot_state::IndexRange, dim: u32) -> plot_state::IndexRange {
    let dim = dim as i32;
    let resolve = |n: i32| -> i32 {
        if n < 0 {
            (dim + n + 1).max(1)
        } else {
            n
        }
    };
    plot_state::IndexRange {
        start: resolve(range.start),
        end: range.end.map(resolve),
    }
}

/// Replace any negative indices in a `GridSubset` with explicit positive
/// 1-based values derived from the cached grid's dimensions.
fn resolve_subset_negatives(
    subset: plot_state::GridSubset,
    cache: &HashMap<String, CachedGrid>,
) -> plot_state::GridSubset {
    let grid_num = subset.grid as usize;
    let dims = cache
        .values()
        .find(|cg| cg.grid_index + 1 == grid_num)
        .map(|cg| cg.grid.dimensions.clone());

    if let Some(dims) = dims {
        plot_state::GridSubset {
            grid: subset.grid,
            gui_managed: subset.gui_managed,
            i_range: subset.i_range.map(|r| resolve_index_range(&r, dims.i)),
            j_range: subset.j_range.map(|r| resolve_index_range(&r, dims.j)),
            k_range: subset.k_range.map(|r| resolve_index_range(&r, dims.k)),
            style: subset.style,
        }
    } else {
        subset
    }
}

/// Convenience command: replace plot subsets via a stable argument shape.
#[tauri::command]
fn set_plot_subsets(subsets: Vec<plot_state::GridSubset>) -> Result<ApplyActionResult, String> {
    let resolved = {
        let cache = GRID_CACHE
            .lock()
            .map_err(|e| format!("Failed to lock grid cache: {e}"))?;
        subsets
            .into_iter()
            .map(|s| resolve_subset_negatives(s, &cache))
            .collect()
    };
    apply_plot_action(PlotAction::SetSubsets(resolved))
}

/// Convenience command: replace plot walls via a stable argument shape.
#[tauri::command]
fn set_plot_walls(walls: Vec<plot_state::GridSubset>) -> Result<ApplyActionResult, String> {
    let resolved = {
        let cache = GRID_CACHE
            .lock()
            .map_err(|e| format!("Failed to lock grid cache: {e}"))?;
        walls
            .into_iter()
            .map(|s| resolve_subset_negatives(s, &cache))
            .collect()
    };
    apply_plot_action(PlotAction::SetWalls(resolved))
}

/// Convenience command: set or clear the FSURFACE numeric
/// specification (value + FUNCTION scalar field).
#[tauri::command]
fn set_plot_fsurface(
    fsurface: Option<plot_state::FsurfaceSpec>,
) -> Result<ApplyActionResult, String> {
    apply_plot_action(PlotAction::SetFsurface(fsurface))
}

/// Convenience command: add one text annotation.
#[tauri::command]
fn add_plot_text_annotation(text: plot_state::PlotText) -> Result<ApplyActionResult, String> {
    apply_plot_action(PlotAction::AddTextAnnotation(text))
}

/// Convenience command: clear all text annotations.
#[tauri::command]
fn clear_plot_text_annotations() -> Result<ApplyActionResult, String> {
    apply_plot_action(PlotAction::ClearTextAnnotations)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ShowStatusResult {
    status: String,
    state: PlotState,
    diagnostics: Vec<plot_state::Diagnostic>,
}

/// Execute SHOW status against current PlotState and return formatted summary.
#[tauri::command]
fn show_plot_status() -> Result<ShowStatusResult, String> {
    let guard = PLOT_STATE
        .lock()
        .map_err(|e| format!("Failed to lock plot state: {e}"))?;
    let current = guard.clone();
    let (state, diagnostics) = apply_action(current, PlotAction::ShowStatus);
    let family = match state.plot_family {
        plot_state::PlotFamily::Contour => "CONTOUR",
        plot_state::PlotFamily::FunctionSurface => "SURFACE/CARPET/LINE",
    };

    let status = format!(
        "SHOW: field={:?}, family={}, axis_view={:?}, text_annotations={}, walls={}, subsets={}",
        state.scalar_field,
        family,
        state.axis_view,
        state.text_annotations.len(),
        state.walls.len(),
        state.subsets.len()
    );
    Ok(ShowStatusResult {
        status,
        state,
        diagnostics,
    })
}

/// Convenience command: set a single manual contour level.
#[tauri::command]
fn set_plot_contour_level(level: f64) -> Result<ApplyActionResult, String> {
    apply_plot_action(PlotAction::SetContourSpec(
        plot_state::ContourSpec::Manual {
            entries: vec![plot_state::ContourEntry {
                value: level,
                color: None,
            }],
        },
    ))
}

/// Convenience command: set the full contour specification (any mode).
#[tauri::command]
fn set_plot_contour_spec(spec: plot_state::ContourSpec) -> Result<ApplyActionResult, String> {
    apply_plot_action(PlotAction::SetContourSpec(spec))
}

/// Convenience command: set the contour visual rendering attribute.
#[tauri::command]
fn set_plot_contour_attribute(
    attribute: plot_state::ContourAttribute,
) -> Result<ApplyActionResult, String> {
    apply_plot_action(PlotAction::SetContourAttribute(attribute))
}

/// Convenience command: commit current plot state boundary.
#[tauri::command]
fn commit_plot() -> Result<ApplyActionResult, String> {
    apply_plot_action(PlotAction::CommitPlot)
}

/// Parse and execute a legacy `.com` file against shared `PlotState`.
///
/// This command applies all parsed actions in order, emits render intents on
/// `PLOT`, captures `SHOW` output, and persists final state into `PLOT_STATE`.
#[tauri::command]
fn execute_com_script(path: String) -> Result<ScriptExecutionResult, String> {
    let parsed = com_parser::parse_com_file(PathBuf::from(&path).as_path())?;

    let mut guard = PLOT_STATE
        .lock()
        .map_err(|e| format!("Failed to lock plot state: {e}"))?;
    let current = guard.clone();
    let result = execute_parsed_script(current, &parsed);
    *guard = result.final_state.clone();
    Ok(result)
}

/// Execute command text entered from the in-app PLOT3D command window.
#[tauri::command]
fn execute_plot3d_commands(commands: String) -> Result<ScriptExecutionResult, String> {
    let mut parsed = com_parser::parse_com_text(&commands, "<command-window>");

    // Resolve negative indices in SetSubsets/SetWalls before applying actions.
    {
        let cache = GRID_CACHE
            .lock()
            .map_err(|e| format!("Failed to lock grid cache: {e}"))?;
        for action in &mut parsed.actions {
            match action {
                PlotAction::SetSubsets(subsets)
                | PlotAction::SetWalls(subsets)
                | PlotAction::AddSubsets(subsets)
                | PlotAction::AddWalls(subsets) => {
                    let resolved: Vec<_> = subsets
                        .drain(..)
                        .map(|s| resolve_subset_negatives(s, &cache))
                        .collect();
                    *subsets = resolved;
                }
                _ => {}
            }
        }
    }

    let mut guard = PLOT_STATE
        .lock()
        .map_err(|e| format!("Failed to lock plot state: {e}"))?;
    let current = guard.clone();
    let result = execute_parsed_script(current, &parsed);
    *guard = result.final_state.clone();
    Ok(result)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging
    logger::init_logger();
    log_info("Application started");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            load_plot3d_file,
            load_plot3d_file_ascii,
            load_plot3d_file_cached,
            load_plot3d_solution,
            load_plot3d_solution_ascii,
            load_plot3d_solution_auto,
            load_plot3d_solution_cached,
            load_plot3d_function,
            convert_grid_to_mesh,
            convert_grid_to_mesh_by_id,
            slice_grid_by_id,
            subset_grid_by_id,
            slice_arbitrary_plane_by_id,
            compute_solution_colors,
            compute_solution_colors_sliced,
            compute_solution_colors_subset_by_id,
            compute_solution_colors_arbitrary_plane,
            get_solution_field_range,
            resolve_contour_levels,
            extract_iso_surface_by_id,
            extract_slice_contours_by_id,
            extract_arbitrary_plane_contours_by_id,
            extract_arbitrary_plane_contours_multi_by_id,
            list_cached_grids,
            list_cached_solutions,
            get_grid_metadata,
            clear_grid_cache,
            clear_solution_cache_v2,
            unload_grid,
            unload_solution,
            open_file_dialog,
            open_multiple_files_dialog,
            detect_file_format,
            get_log_entries,
            clear_log_entries,
            export_logs_to_file,
            save_log_file_dialog,
            write_text_file,
            save_png_file_dialog,
            write_png_file,
            frontend_log,
            open_about_window,
            open_probe_window,
            close_probe_window,
            update_probe_window_html,
            get_plot_state,
            apply_plot_action,
            set_plot_scalar_field,
            set_plot_family,
            set_plot_viewpoint,
            set_plot_axis_view,
            set_plot_subsets,
            set_plot_walls,
            set_plot_fsurface,
            add_plot_text_annotation,
            clear_plot_text_annotations,
            set_plot_contour_level,
            set_plot_contour_spec,
            set_plot_contour_attribute,
            show_plot_status,
            commit_plot,
            execute_com_script,
            execute_plot3d_commands,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
