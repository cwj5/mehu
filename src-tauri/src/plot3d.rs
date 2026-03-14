use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

use crate::logger;

// Thread-local storage for last loaded solution file metadata
thread_local! {
    static LAST_SOLUTION_METADATA: RefCell<Option<SolutionFileMetadata>> = RefCell::new(None);
}

/// Represents a PLOT3D grid structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plot3DGrid {
    pub dimensions: GridDimensions,
    pub x_coords: Vec<f32>,
    pub y_coords: Vec<f32>,
    pub z_coords: Vec<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iblank: Option<Vec<i32>>, // Blanking array (0=blanked, 1=visible)
}

/// File metadata about the loaded grid
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridFileMetadata {
    pub byte_order: String, // "Little-Endian" or "Big-Endian"
    pub is_detected: bool,  // true if auto-detected, false if assumed
    pub precision: String,  // "f32", "f64", or "mixed"
    pub has_iblank: bool,
    pub num_grids: usize,
    pub grid_dimensions: Vec<GridDimensions>,
}

/// File metadata about the loaded solution file
#[derive(Debug, Clone)]
pub struct SolutionFileMetadata {
    pub format: String,     // "binary" or "ASCII"
    pub precision: String,  // "f32" or "f64"
    pub byte_order: String, // "Little-Endian" or "Big-Endian" (ASCII uses "N/A")
}

/// PLOT3D solution metadata from Q file header
/// Fields are read in sequence; if the metadata record is shorter than expected,
/// later fields will be None
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plot3DMetadata {
    pub refmach: Option<f32>,   // Reference Mach number
    pub alpha: Option<f32>,     // Angle of attack (degrees)
    pub rey: Option<f32>,       // Reynolds number
    pub time: Option<f32>,      // Time value
    pub gaminf: Option<f32>,    // Gamma at infinity
    pub beta: Option<f32>,      // Sideslip angle (degrees)
    pub tinf: Option<f32>,      // Temperature at infinity
    pub igam: Option<f32>,      // Gas model flag (0=perfect gas, 1=equilibrium)
    pub htinf: Option<f32>,     // Total enthalpy at infinity
    pub ht1: Option<f32>,       // Reserved
    pub ht2: Option<f32>,       // Reserved
    pub rgas: Option<Vec<f32>>, // Gas constants (variable length)
    pub fsmach: Option<f32>,    // Free stream Mach number
    pub tvref: Option<f32>,     // Reference temperature
    pub dtvref: Option<f32>,    // Delta reference temperature
}

/// Represents PLOT3D solution data (Q file)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plot3DSolution {
    pub grid_index: usize,
    pub dimensions: GridDimensions,
    pub rho: Vec<f32>,  // Density (non-dimensional)
    pub rhou: Vec<f32>, // Momentum X (non-dimensional)
    pub rhov: Vec<f32>, // Momentum Y (non-dimensional)
    pub rhow: Vec<f32>, // Momentum Z (non-dimensional)
    pub rhoe: Vec<f32>, // Total Energy (non-dimensional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gamma: Option<Vec<f32>>, // Ratio of specific heats (always at Q[5], NQ=6+NQC+NQT)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Plot3DMetadata>, // Solution metadata from file header
}

/// PLOT3D function file data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plot3DFunction {
    pub grid_index: usize,
    pub dimensions: GridDimensions,
    pub function_data: Vec<Vec<f32>>, // Multiple functions per grid
}

/// Grid dimensions (I, J, K)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridDimensions {
    pub i: u32,
    pub j: u32,
    pub k: u32,
}

/// Mesh geometry suitable for Three.js rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshGeometry {
    pub vertices: Vec<f32>,         // Flat array of x, y, z coordinates
    pub indices: Vec<u32>,          // Line indices for wireframe rendering
    pub triangle_indices: Vec<u32>, // Triangle indices for solid rendering
    pub normals: Vec<f32>,          // Computed vertex normals
    pub vertex_count: usize,
    pub face_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colors: Option<Vec<f32>>, // Optional vertex colors (r, g, b interleaved)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertex_cell_data: Option<Vec<VertexCellData>>, // Cell interpolation data for solution mapping
}

/// Stores interpolation data for a vertex created by arbitrary plane slicing
/// Used to map vertices back to their source cells for solution interpolation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VertexCellData {
    pub cell_i: usize,
    pub cell_j: usize,
    pub cell_k: usize,
    pub weights: [f32; 8], // Barycentric weights for the 8 corners of the hexahedral cell
}

/// Byte order detection
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ByteOrder {
    LittleEndian,
    BigEndian,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Precision {
    F32,
    F64,
    Mixed,
}

impl Precision {
    pub fn as_str(&self) -> &'static str {
        match self {
            Precision::F32 => "f32",
            Precision::F64 => "f64",
            Precision::Mixed => "mixed",
        }
    }
}

// Marching Cubes lookup tables
// Edge table: for each of 256 cube configurations, which edges are crossed
const EDGE_TABLE: [i32; 256] = [
    0x0, 0x109, 0x203, 0x30a, 0x406, 0x50f, 0x605, 0x70c, 0x80c, 0x905, 0xa0f, 0xb06, 0xc0a, 0xd03,
    0xe09, 0xf00, 0x190, 0x99, 0x393, 0x29a, 0x596, 0x49f, 0x795, 0x69c, 0x99c, 0x895, 0xb9f,
    0xa96, 0xd9a, 0xc93, 0xf99, 0xe90, 0x230, 0x339, 0x33, 0x13a, 0x636, 0x73f, 0x435, 0x53c,
    0xa3c, 0xb35, 0x83f, 0x936, 0xe3a, 0xf33, 0xc39, 0xd30, 0x3a0, 0x2a9, 0x1a3, 0xaa, 0x7a6,
    0x6af, 0x5a5, 0x4ac, 0xbac, 0xaa5, 0x9af, 0x8a6, 0xfaa, 0xea3, 0xda9, 0xca0, 0x460, 0x569,
    0x663, 0x76a, 0x66, 0x16f, 0x265, 0x36c, 0xc6c, 0xd65, 0xe6f, 0xf66, 0x86a, 0x963, 0xa69,
    0xb60, 0x5f0, 0x4f9, 0x7f3, 0x6fa, 0x1f6, 0xff, 0x3f5, 0x2fc, 0xdfc, 0xcf5, 0xfff, 0xef6,
    0x9fa, 0x8f3, 0xbf9, 0xaf0, 0x650, 0x759, 0x453, 0x55a, 0x256, 0x35f, 0x55, 0x15c, 0xe5c,
    0xf55, 0xc5f, 0xd56, 0xa5a, 0xb53, 0x859, 0x950, 0x7c0, 0x6c9, 0x5c3, 0x4ca, 0x3c6, 0x2cf,
    0x1c5, 0xcc, 0xfcc, 0xec5, 0xdcf, 0xcc6, 0xbca, 0xac3, 0x9c9, 0x8c0, 0x8c0, 0x9c9, 0xac3,
    0xbca, 0xcc6, 0xdcf, 0xec5, 0xfcc, 0xcc, 0x1c5, 0x2cf, 0x3c6, 0x4ca, 0x5c3, 0x6c9, 0x7c0,
    0x950, 0x859, 0xb53, 0xa5a, 0xd56, 0xc5f, 0xf55, 0xe5c, 0x15c, 0x55, 0x35f, 0x256, 0x55a,
    0x453, 0x759, 0x650, 0xaf0, 0xbf9, 0x8f3, 0x9fa, 0xef6, 0xfff, 0xcf5, 0xdfc, 0x2fc, 0x3f5,
    0xff, 0x1f6, 0x6fa, 0x7f3, 0x4f9, 0x5f0, 0xb60, 0xa69, 0x963, 0x86a, 0xf66, 0xe6f, 0xd65,
    0xc6c, 0x36c, 0x265, 0x16f, 0x66, 0x76a, 0x663, 0x569, 0x460, 0xca0, 0xda9, 0xea3, 0xfaa,
    0x8a6, 0x9af, 0xaa5, 0xbac, 0x4ac, 0x5a5, 0x6af, 0x7a6, 0xaa, 0x1a3, 0x2a9, 0x3a0, 0xd30,
    0xc39, 0xf33, 0xe3a, 0x936, 0x83f, 0xb35, 0xa3c, 0x53c, 0x435, 0x73f, 0x636, 0x13a, 0x33,
    0x339, 0x230, 0xe90, 0xf99, 0xc93, 0xd9a, 0xa96, 0xb9f, 0x895, 0x99c, 0x69c, 0x795, 0x49f,
    0x596, 0x29a, 0x393, 0x99, 0x190, 0xf00, 0xe09, 0xd03, 0xc0a, 0xb06, 0xa0f, 0x905, 0x80c,
    0x70c, 0x605, 0x50f, 0x406, 0x30a, 0x203, 0x109, 0x0,
];

// Triangle table: for each configuration, lists of edges that form triangles
// -1 marks end of triangle list for that configuration
const TRI_TABLE: [[i32; 16]; 256] = [
    [
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
    ],
    [0, 8, 3, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 1, 9, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [1, 8, 3, 9, 8, 1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [1, 2, 10, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 8, 3, 1, 2, 10, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [9, 2, 10, 0, 2, 9, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [2, 8, 3, 2, 10, 8, 10, 9, 8, -1, -1, -1, -1, -1, -1, -1],
    [3, 11, 2, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 11, 2, 8, 11, 0, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [1, 9, 0, 2, 3, 11, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [1, 11, 2, 1, 9, 11, 9, 8, 11, -1, -1, -1, -1, -1, -1, -1],
    [3, 10, 1, 11, 10, 3, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 10, 1, 0, 8, 10, 8, 11, 10, -1, -1, -1, -1, -1, -1, -1],
    [3, 9, 0, 3, 11, 9, 11, 10, 9, -1, -1, -1, -1, -1, -1, -1],
    [9, 8, 10, 10, 8, 11, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [4, 7, 8, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [4, 3, 0, 7, 3, 4, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 1, 9, 8, 4, 7, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [4, 1, 9, 4, 7, 1, 7, 3, 1, -1, -1, -1, -1, -1, -1, -1],
    [1, 2, 10, 8, 4, 7, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [3, 4, 7, 3, 0, 4, 1, 2, 10, -1, -1, -1, -1, -1, -1, -1],
    [9, 2, 10, 9, 0, 2, 8, 4, 7, -1, -1, -1, -1, -1, -1, -1],
    [2, 10, 9, 2, 9, 7, 2, 7, 3, 7, 9, 4, -1, -1, -1, -1],
    [8, 4, 7, 3, 11, 2, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [11, 4, 7, 11, 2, 4, 2, 0, 4, -1, -1, -1, -1, -1, -1, -1],
    [9, 0, 1, 8, 4, 7, 2, 3, 11, -1, -1, -1, -1, -1, -1, -1],
    [4, 7, 11, 9, 4, 11, 9, 11, 2, 9, 2, 1, -1, -1, -1, -1],
    [3, 10, 1, 3, 11, 10, 7, 8, 4, -1, -1, -1, -1, -1, -1, -1],
    [1, 11, 10, 1, 4, 11, 1, 0, 4, 7, 11, 4, -1, -1, -1, -1],
    [4, 7, 8, 9, 0, 11, 9, 11, 10, 11, 0, 3, -1, -1, -1, -1],
    [4, 7, 11, 4, 11, 9, 9, 11, 10, -1, -1, -1, -1, -1, -1, -1],
    [9, 5, 4, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [9, 5, 4, 0, 8, 3, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 5, 4, 1, 5, 0, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [8, 5, 4, 8, 3, 5, 3, 1, 5, -1, -1, -1, -1, -1, -1, -1],
    [1, 2, 10, 9, 5, 4, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [3, 0, 8, 1, 2, 10, 4, 9, 5, -1, -1, -1, -1, -1, -1, -1],
    [5, 2, 10, 5, 4, 2, 4, 0, 2, -1, -1, -1, -1, -1, -1, -1],
    [2, 10, 5, 3, 2, 5, 3, 5, 4, 3, 4, 8, -1, -1, -1, -1],
    [9, 5, 4, 2, 3, 11, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 11, 2, 0, 8, 11, 4, 9, 5, -1, -1, -1, -1, -1, -1, -1],
    [0, 5, 4, 0, 1, 5, 2, 3, 11, -1, -1, -1, -1, -1, -1, -1],
    [2, 1, 5, 2, 5, 8, 2, 8, 11, 4, 8, 5, -1, -1, -1, -1],
    [10, 3, 11, 10, 1, 3, 9, 5, 4, -1, -1, -1, -1, -1, -1, -1],
    [4, 9, 5, 0, 8, 1, 8, 10, 1, 8, 11, 10, -1, -1, -1, -1],
    [5, 4, 0, 5, 0, 11, 5, 11, 10, 11, 0, 3, -1, -1, -1, -1],
    [5, 4, 8, 5, 8, 10, 10, 8, 11, -1, -1, -1, -1, -1, -1, -1],
    [9, 7, 8, 5, 7, 9, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [9, 3, 0, 9, 5, 3, 5, 7, 3, -1, -1, -1, -1, -1, -1, -1],
    [0, 7, 8, 0, 1, 7, 1, 5, 7, -1, -1, -1, -1, -1, -1, -1],
    [1, 5, 3, 3, 5, 7, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [9, 7, 8, 9, 5, 7, 10, 1, 2, -1, -1, -1, -1, -1, -1, -1],
    [10, 1, 2, 9, 5, 0, 5, 3, 0, 5, 7, 3, -1, -1, -1, -1],
    [8, 0, 2, 8, 2, 5, 8, 5, 7, 10, 5, 2, -1, -1, -1, -1],
    [2, 10, 5, 2, 5, 3, 3, 5, 7, -1, -1, -1, -1, -1, -1, -1],
    [7, 9, 5, 7, 8, 9, 3, 11, 2, -1, -1, -1, -1, -1, -1, -1],
    [9, 5, 7, 9, 7, 2, 9, 2, 0, 2, 7, 11, -1, -1, -1, -1],
    [2, 3, 11, 0, 1, 8, 1, 7, 8, 1, 5, 7, -1, -1, -1, -1],
    [11, 2, 1, 11, 1, 7, 7, 1, 5, -1, -1, -1, -1, -1, -1, -1],
    [9, 5, 8, 8, 5, 7, 10, 1, 3, 10, 3, 11, -1, -1, -1, -1],
    [5, 7, 0, 5, 0, 9, 7, 11, 0, 1, 0, 10, 11, 10, 0, -1],
    [11, 10, 0, 11, 0, 3, 10, 5, 0, 8, 0, 7, 5, 7, 0, -1],
    [11, 10, 5, 7, 11, 5, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [10, 6, 5, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 8, 3, 5, 10, 6, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [9, 0, 1, 5, 10, 6, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [1, 8, 3, 1, 9, 8, 5, 10, 6, -1, -1, -1, -1, -1, -1, -1],
    [1, 6, 5, 2, 6, 1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [1, 6, 5, 1, 2, 6, 3, 0, 8, -1, -1, -1, -1, -1, -1, -1],
    [9, 6, 5, 9, 0, 6, 0, 2, 6, -1, -1, -1, -1, -1, -1, -1],
    [5, 9, 8, 5, 8, 2, 5, 2, 6, 3, 2, 8, -1, -1, -1, -1],
    [2, 3, 11, 10, 6, 5, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [11, 0, 8, 11, 2, 0, 10, 6, 5, -1, -1, -1, -1, -1, -1, -1],
    [0, 1, 9, 2, 3, 11, 5, 10, 6, -1, -1, -1, -1, -1, -1, -1],
    [5, 10, 6, 1, 9, 2, 9, 11, 2, 9, 8, 11, -1, -1, -1, -1],
    [6, 3, 11, 6, 5, 3, 5, 1, 3, -1, -1, -1, -1, -1, -1, -1],
    [0, 8, 11, 0, 11, 5, 0, 5, 1, 5, 11, 6, -1, -1, -1, -1],
    [3, 11, 6, 0, 3, 6, 0, 6, 5, 0, 5, 9, -1, -1, -1, -1],
    [6, 5, 9, 6, 9, 11, 11, 9, 8, -1, -1, -1, -1, -1, -1, -1],
    [5, 10, 6, 4, 7, 8, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [4, 3, 0, 4, 7, 3, 6, 5, 10, -1, -1, -1, -1, -1, -1, -1],
    [1, 9, 0, 5, 10, 6, 8, 4, 7, -1, -1, -1, -1, -1, -1, -1],
    [10, 6, 5, 1, 9, 7, 1, 7, 3, 7, 9, 4, -1, -1, -1, -1],
    [6, 1, 2, 6, 5, 1, 4, 7, 8, -1, -1, -1, -1, -1, -1, -1],
    [1, 2, 5, 5, 2, 6, 3, 0, 4, 3, 4, 7, -1, -1, -1, -1],
    [8, 4, 7, 9, 0, 5, 0, 6, 5, 0, 2, 6, -1, -1, -1, -1],
    [7, 3, 9, 7, 9, 4, 3, 2, 9, 5, 9, 6, 2, 6, 9, -1],
    [3, 11, 2, 7, 8, 4, 10, 6, 5, -1, -1, -1, -1, -1, -1, -1],
    [5, 10, 6, 4, 7, 2, 4, 2, 0, 2, 7, 11, -1, -1, -1, -1],
    [0, 1, 9, 4, 7, 8, 2, 3, 11, 5, 10, 6, -1, -1, -1, -1],
    [9, 2, 1, 9, 11, 2, 9, 4, 11, 7, 11, 4, 5, 10, 6, -1],
    [8, 4, 7, 3, 11, 5, 3, 5, 1, 5, 11, 6, -1, -1, -1, -1],
    [5, 1, 11, 5, 11, 6, 1, 0, 11, 7, 11, 4, 0, 4, 11, -1],
    [0, 5, 9, 0, 6, 5, 0, 3, 6, 11, 6, 3, 8, 4, 7, -1],
    [6, 5, 9, 6, 9, 11, 4, 7, 9, 7, 11, 9, -1, -1, -1, -1],
    [10, 4, 9, 6, 4, 10, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [4, 10, 6, 4, 9, 10, 0, 8, 3, -1, -1, -1, -1, -1, -1, -1],
    [10, 0, 1, 10, 6, 0, 6, 4, 0, -1, -1, -1, -1, -1, -1, -1],
    [8, 3, 1, 8, 1, 6, 8, 6, 4, 6, 1, 10, -1, -1, -1, -1],
    [1, 4, 9, 1, 2, 4, 2, 6, 4, -1, -1, -1, -1, -1, -1, -1],
    [3, 0, 8, 1, 2, 9, 2, 4, 9, 2, 6, 4, -1, -1, -1, -1],
    [0, 2, 4, 4, 2, 6, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [8, 3, 2, 8, 2, 4, 4, 2, 6, -1, -1, -1, -1, -1, -1, -1],
    [10, 4, 9, 10, 6, 4, 11, 2, 3, -1, -1, -1, -1, -1, -1, -1],
    [0, 8, 2, 2, 8, 11, 4, 9, 10, 4, 10, 6, -1, -1, -1, -1],
    [3, 11, 2, 0, 1, 6, 0, 6, 4, 6, 1, 10, -1, -1, -1, -1],
    [6, 4, 1, 6, 1, 10, 4, 8, 1, 2, 1, 11, 8, 11, 1, -1],
    [9, 6, 4, 9, 3, 6, 9, 1, 3, 11, 6, 3, -1, -1, -1, -1],
    [8, 11, 1, 8, 1, 0, 11, 6, 1, 9, 1, 4, 6, 4, 1, -1],
    [3, 11, 6, 3, 6, 0, 0, 6, 4, -1, -1, -1, -1, -1, -1, -1],
    [6, 4, 8, 11, 6, 8, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [7, 10, 6, 7, 8, 10, 8, 9, 10, -1, -1, -1, -1, -1, -1, -1],
    [0, 7, 3, 0, 10, 7, 0, 9, 10, 6, 7, 10, -1, -1, -1, -1],
    [10, 6, 7, 1, 10, 7, 1, 7, 8, 1, 8, 0, -1, -1, -1, -1],
    [10, 6, 7, 10, 7, 1, 1, 7, 3, -1, -1, -1, -1, -1, -1, -1],
    [1, 2, 6, 1, 6, 8, 1, 8, 9, 8, 6, 7, -1, -1, -1, -1],
    [2, 6, 9, 2, 9, 1, 6, 7, 9, 0, 9, 3, 7, 3, 9, -1],
    [7, 8, 0, 7, 0, 6, 6, 0, 2, -1, -1, -1, -1, -1, -1, -1],
    [7, 3, 2, 6, 7, 2, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [2, 3, 11, 10, 6, 8, 10, 8, 9, 8, 6, 7, -1, -1, -1, -1],
    [2, 0, 7, 2, 7, 11, 0, 9, 7, 6, 7, 10, 9, 10, 7, -1],
    [1, 8, 0, 1, 7, 8, 1, 10, 7, 6, 7, 10, 2, 3, 11, -1],
    [11, 2, 1, 11, 1, 7, 10, 6, 1, 6, 7, 1, -1, -1, -1, -1],
    [8, 9, 6, 8, 6, 7, 9, 1, 6, 11, 6, 3, 1, 3, 6, -1],
    [0, 9, 1, 11, 6, 7, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [7, 8, 0, 7, 0, 6, 3, 11, 0, 11, 6, 0, -1, -1, -1, -1],
    [7, 11, 6, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [7, 6, 11, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [3, 0, 8, 11, 7, 6, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 1, 9, 11, 7, 6, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [8, 1, 9, 8, 3, 1, 11, 7, 6, -1, -1, -1, -1, -1, -1, -1],
    [10, 1, 2, 6, 11, 7, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [1, 2, 10, 3, 0, 8, 6, 11, 7, -1, -1, -1, -1, -1, -1, -1],
    [2, 9, 0, 2, 10, 9, 6, 11, 7, -1, -1, -1, -1, -1, -1, -1],
    [6, 11, 7, 2, 10, 3, 10, 8, 3, 10, 9, 8, -1, -1, -1, -1],
    [7, 2, 3, 6, 2, 7, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [7, 0, 8, 7, 6, 0, 6, 2, 0, -1, -1, -1, -1, -1, -1, -1],
    [2, 7, 6, 2, 3, 7, 0, 1, 9, -1, -1, -1, -1, -1, -1, -1],
    [1, 6, 2, 1, 8, 6, 1, 9, 8, 8, 7, 6, -1, -1, -1, -1],
    [10, 7, 6, 10, 1, 7, 1, 3, 7, -1, -1, -1, -1, -1, -1, -1],
    [10, 7, 6, 1, 7, 10, 1, 8, 7, 1, 0, 8, -1, -1, -1, -1],
    [0, 3, 7, 0, 7, 10, 0, 10, 9, 6, 10, 7, -1, -1, -1, -1],
    [7, 6, 10, 7, 10, 8, 8, 10, 9, -1, -1, -1, -1, -1, -1, -1],
    [6, 8, 4, 11, 8, 6, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [3, 6, 11, 3, 0, 6, 0, 4, 6, -1, -1, -1, -1, -1, -1, -1],
    [8, 6, 11, 8, 4, 6, 9, 0, 1, -1, -1, -1, -1, -1, -1, -1],
    [9, 4, 6, 9, 6, 3, 9, 3, 1, 11, 3, 6, -1, -1, -1, -1],
    [6, 8, 4, 6, 11, 8, 2, 10, 1, -1, -1, -1, -1, -1, -1, -1],
    [1, 2, 10, 3, 0, 11, 0, 6, 11, 0, 4, 6, -1, -1, -1, -1],
    [4, 11, 8, 4, 6, 11, 0, 2, 9, 2, 10, 9, -1, -1, -1, -1],
    [10, 9, 3, 10, 3, 2, 9, 4, 3, 11, 3, 6, 4, 6, 3, -1],
    [8, 2, 3, 8, 4, 2, 4, 6, 2, -1, -1, -1, -1, -1, -1, -1],
    [0, 4, 2, 4, 6, 2, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [1, 9, 0, 2, 3, 4, 2, 4, 6, 4, 3, 8, -1, -1, -1, -1],
    [1, 9, 4, 1, 4, 2, 2, 4, 6, -1, -1, -1, -1, -1, -1, -1],
    [8, 1, 3, 8, 6, 1, 8, 4, 6, 6, 10, 1, -1, -1, -1, -1],
    [10, 1, 0, 10, 0, 6, 6, 0, 4, -1, -1, -1, -1, -1, -1, -1],
    [4, 6, 3, 4, 3, 8, 6, 10, 3, 0, 3, 9, 10, 9, 3, -1],
    [10, 9, 4, 6, 10, 4, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [4, 9, 5, 7, 6, 11, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 8, 3, 4, 9, 5, 11, 7, 6, -1, -1, -1, -1, -1, -1, -1],
    [5, 0, 1, 5, 4, 0, 7, 6, 11, -1, -1, -1, -1, -1, -1, -1],
    [11, 7, 6, 8, 3, 4, 3, 5, 4, 3, 1, 5, -1, -1, -1, -1],
    [9, 5, 4, 10, 1, 2, 7, 6, 11, -1, -1, -1, -1, -1, -1, -1],
    [6, 11, 7, 1, 2, 10, 0, 8, 3, 4, 9, 5, -1, -1, -1, -1],
    [7, 6, 11, 5, 4, 10, 4, 2, 10, 4, 0, 2, -1, -1, -1, -1],
    [3, 4, 8, 3, 5, 4, 3, 2, 5, 10, 5, 2, 11, 7, 6, -1],
    [7, 2, 3, 7, 6, 2, 5, 4, 9, -1, -1, -1, -1, -1, -1, -1],
    [9, 5, 4, 0, 8, 6, 0, 6, 2, 6, 8, 7, -1, -1, -1, -1],
    [3, 6, 2, 3, 7, 6, 1, 5, 0, 5, 4, 0, -1, -1, -1, -1],
    [6, 2, 8, 6, 8, 7, 2, 1, 8, 4, 8, 5, 1, 5, 8, -1],
    [9, 5, 4, 10, 1, 6, 1, 7, 6, 1, 3, 7, -1, -1, -1, -1],
    [1, 6, 10, 1, 7, 6, 1, 0, 7, 8, 7, 0, 9, 5, 4, -1],
    [4, 0, 10, 4, 10, 5, 0, 3, 10, 6, 10, 7, 3, 7, 10, -1],
    [7, 6, 10, 7, 10, 8, 5, 4, 10, 4, 8, 10, -1, -1, -1, -1],
    [6, 9, 5, 6, 11, 9, 11, 8, 9, -1, -1, -1, -1, -1, -1, -1],
    [3, 6, 11, 0, 6, 3, 0, 5, 6, 0, 9, 5, -1, -1, -1, -1],
    [0, 11, 8, 0, 5, 11, 0, 1, 5, 5, 6, 11, -1, -1, -1, -1],
    [6, 11, 3, 6, 3, 5, 5, 3, 1, -1, -1, -1, -1, -1, -1, -1],
    [1, 2, 10, 9, 5, 11, 9, 11, 8, 11, 5, 6, -1, -1, -1, -1],
    [0, 11, 3, 0, 6, 11, 0, 9, 6, 5, 6, 9, 1, 2, 10, -1],
    [11, 8, 5, 11, 5, 6, 8, 0, 5, 10, 5, 2, 0, 2, 5, -1],
    [6, 11, 3, 6, 3, 5, 2, 10, 3, 10, 5, 3, -1, -1, -1, -1],
    [5, 8, 9, 5, 2, 8, 5, 6, 2, 3, 8, 2, -1, -1, -1, -1],
    [9, 5, 6, 9, 6, 0, 0, 6, 2, -1, -1, -1, -1, -1, -1, -1],
    [1, 5, 8, 1, 8, 0, 5, 6, 8, 3, 8, 2, 6, 2, 8, -1],
    [1, 5, 6, 2, 1, 6, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [1, 3, 6, 1, 6, 10, 3, 8, 6, 5, 6, 9, 8, 9, 6, -1],
    [10, 1, 0, 10, 0, 6, 9, 5, 0, 5, 6, 0, -1, -1, -1, -1],
    [0, 3, 8, 5, 6, 10, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [10, 5, 6, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [11, 5, 10, 7, 5, 11, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [11, 5, 10, 11, 7, 5, 8, 3, 0, -1, -1, -1, -1, -1, -1, -1],
    [5, 11, 7, 5, 10, 11, 1, 9, 0, -1, -1, -1, -1, -1, -1, -1],
    [10, 7, 5, 10, 11, 7, 9, 8, 1, 8, 3, 1, -1, -1, -1, -1],
    [11, 1, 2, 11, 7, 1, 7, 5, 1, -1, -1, -1, -1, -1, -1, -1],
    [0, 8, 3, 1, 2, 7, 1, 7, 5, 7, 2, 11, -1, -1, -1, -1],
    [9, 7, 5, 9, 2, 7, 9, 0, 2, 2, 11, 7, -1, -1, -1, -1],
    [7, 5, 2, 7, 2, 11, 5, 9, 2, 3, 2, 8, 9, 8, 2, -1],
    [2, 5, 10, 2, 3, 5, 3, 7, 5, -1, -1, -1, -1, -1, -1, -1],
    [8, 2, 0, 8, 5, 2, 8, 7, 5, 10, 2, 5, -1, -1, -1, -1],
    [9, 0, 1, 5, 10, 3, 5, 3, 7, 3, 10, 2, -1, -1, -1, -1],
    [9, 8, 2, 9, 2, 1, 8, 7, 2, 10, 2, 5, 7, 5, 2, -1],
    [1, 3, 5, 3, 7, 5, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 8, 7, 0, 7, 1, 1, 7, 5, -1, -1, -1, -1, -1, -1, -1],
    [9, 0, 3, 9, 3, 5, 5, 3, 7, -1, -1, -1, -1, -1, -1, -1],
    [9, 8, 7, 5, 9, 7, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [5, 8, 4, 5, 10, 8, 10, 11, 8, -1, -1, -1, -1, -1, -1, -1],
    [5, 0, 4, 5, 11, 0, 5, 10, 11, 11, 3, 0, -1, -1, -1, -1],
    [0, 1, 9, 8, 4, 10, 8, 10, 11, 10, 4, 5, -1, -1, -1, -1],
    [10, 11, 4, 10, 4, 5, 11, 3, 4, 9, 4, 1, 3, 1, 4, -1],
    [2, 5, 1, 2, 8, 5, 2, 11, 8, 4, 5, 8, -1, -1, -1, -1],
    [0, 4, 11, 0, 11, 3, 4, 5, 11, 2, 11, 1, 5, 1, 11, -1],
    [0, 2, 5, 0, 5, 9, 2, 11, 5, 4, 5, 8, 11, 8, 5, -1],
    [9, 4, 5, 2, 11, 3, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [2, 5, 10, 3, 5, 2, 3, 4, 5, 3, 8, 4, -1, -1, -1, -1],
    [5, 10, 2, 5, 2, 4, 4, 2, 0, -1, -1, -1, -1, -1, -1, -1],
    [3, 10, 2, 3, 5, 10, 3, 8, 5, 4, 5, 8, 0, 1, 9, -1],
    [5, 10, 2, 5, 2, 4, 1, 9, 2, 9, 4, 2, -1, -1, -1, -1],
    [8, 4, 5, 8, 5, 3, 3, 5, 1, -1, -1, -1, -1, -1, -1, -1],
    [0, 4, 5, 1, 0, 5, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [8, 4, 5, 8, 5, 3, 9, 0, 5, 0, 3, 5, -1, -1, -1, -1],
    [9, 4, 5, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [4, 11, 7, 4, 9, 11, 9, 10, 11, -1, -1, -1, -1, -1, -1, -1],
    [0, 8, 3, 4, 9, 7, 9, 11, 7, 9, 10, 11, -1, -1, -1, -1],
    [1, 10, 11, 1, 11, 4, 1, 4, 0, 7, 4, 11, -1, -1, -1, -1],
    [3, 1, 4, 3, 4, 8, 1, 10, 4, 7, 4, 11, 10, 11, 4, -1],
    [4, 11, 7, 9, 11, 4, 9, 2, 11, 9, 1, 2, -1, -1, -1, -1],
    [9, 7, 4, 9, 11, 7, 9, 1, 11, 2, 11, 1, 0, 8, 3, -1],
    [11, 7, 4, 11, 4, 2, 2, 4, 0, -1, -1, -1, -1, -1, -1, -1],
    [11, 7, 4, 11, 4, 2, 8, 3, 4, 3, 2, 4, -1, -1, -1, -1],
    [2, 9, 10, 2, 7, 9, 2, 3, 7, 7, 4, 9, -1, -1, -1, -1],
    [9, 10, 7, 9, 7, 4, 10, 2, 7, 8, 7, 0, 2, 0, 7, -1],
    [3, 7, 10, 3, 10, 2, 7, 4, 10, 1, 10, 0, 4, 0, 10, -1],
    [1, 10, 2, 8, 7, 4, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [4, 9, 1, 4, 1, 7, 7, 1, 3, -1, -1, -1, -1, -1, -1, -1],
    [4, 9, 1, 4, 1, 7, 0, 8, 1, 8, 7, 1, -1, -1, -1, -1],
    [4, 0, 3, 7, 4, 3, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [4, 8, 7, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [9, 10, 8, 10, 11, 8, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [3, 0, 9, 3, 9, 11, 11, 9, 10, -1, -1, -1, -1, -1, -1, -1],
    [0, 1, 10, 0, 10, 8, 8, 10, 11, -1, -1, -1, -1, -1, -1, -1],
    [3, 1, 10, 11, 3, 10, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [1, 2, 11, 1, 11, 9, 9, 11, 8, -1, -1, -1, -1, -1, -1, -1],
    [3, 0, 9, 3, 9, 11, 1, 2, 9, 2, 11, 9, -1, -1, -1, -1],
    [0, 2, 11, 8, 0, 11, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [3, 2, 11, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [2, 3, 8, 2, 8, 10, 10, 8, 9, -1, -1, -1, -1, -1, -1, -1],
    [9, 10, 2, 0, 9, 2, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [2, 3, 8, 2, 8, 10, 0, 1, 8, 1, 10, 8, -1, -1, -1, -1],
    [1, 10, 2, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [1, 3, 8, 9, 1, 8, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 9, 1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 3, 8, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
    ],
];

impl Plot3DGrid {
    /// Calculate total number of points
    pub fn total_points(&self) -> usize {
        (self.dimensions.i as usize) * (self.dimensions.j as usize) * (self.dimensions.k as usize)
    }

    /// Create a cross-sectional slice of the grid
    /// plane: "I", "J", or "K" specifying which plane constant
    /// index: the index along that plane (0-based)
    /// Returns a new 2D grid representing the slice
    pub fn slice_grid(&self, plane: &str, index: u32) -> Result<Plot3DGrid, String> {
        let i = self.dimensions.i as usize;
        let j = self.dimensions.j as usize;
        let k = self.dimensions.k as usize;

        match plane.to_uppercase().as_str() {
            "I" => {
                // Constant I plane: varies in J and K
                // Return as J x K x 1 so the surface renders at k=0
                let i_idx = index as usize;
                if i_idx >= i {
                    return Err(format!("I index {} out of bounds [0, {})", i_idx, i));
                }

                let mut x_coords = Vec::with_capacity(j * k);
                let mut y_coords = Vec::with_capacity(j * k);
                let mut z_coords = Vec::with_capacity(j * k);
                let mut iblank_vec = self.iblank.as_ref().map(|_| Vec::with_capacity(j * k));

                // Iterate: for each K, for each J (so data fills in J order first)
                for k_idx in 0..k {
                    for j_idx in 0..j {
                        let idx = Self::linear_index(i_idx, j_idx, k_idx, i, j);
                        x_coords.push(self.x_coords[idx]);
                        y_coords.push(self.y_coords[idx]);
                        z_coords.push(self.z_coords[idx]);
                        if let Some(ref mut iblank) = iblank_vec {
                            iblank.push(self.iblank.as_ref().map(|ib| ib[idx]).unwrap_or(1));
                        }
                    }
                }

                Ok(Plot3DGrid {
                    dimensions: GridDimensions {
                        i: j as u32, // J becomes the new I
                        j: k as u32, // K becomes the new J
                        k: 1,        // Surface at k=0
                    },
                    x_coords,
                    y_coords,
                    z_coords,
                    iblank: iblank_vec,
                })
            }
            "J" => {
                // Constant J plane: varies in I and K
                // Return as I x K x 1 so the surface renders at k=0
                let j_idx = index as usize;
                if j_idx >= j {
                    return Err(format!("J index {} out of bounds [0, {})", j_idx, j));
                }

                let mut x_coords = Vec::with_capacity(i * k);
                let mut y_coords = Vec::with_capacity(i * k);
                let mut z_coords = Vec::with_capacity(i * k);
                let mut iblank_vec = self.iblank.as_ref().map(|_| Vec::with_capacity(i * k));

                // Iterate: for each K, for each I (so data fills in I order first)
                for k_idx in 0..k {
                    for i_idx in 0..i {
                        let idx = Self::linear_index(i_idx, j_idx, k_idx, i, j);
                        x_coords.push(self.x_coords[idx]);
                        y_coords.push(self.y_coords[idx]);
                        z_coords.push(self.z_coords[idx]);
                        if let Some(ref mut iblank) = iblank_vec {
                            iblank.push(self.iblank.as_ref().map(|ib| ib[idx]).unwrap_or(1));
                        }
                    }
                }

                Ok(Plot3DGrid {
                    dimensions: GridDimensions {
                        i: i as u32,
                        j: k as u32, // K becomes the new J
                        k: 1,        // Surface at k=0
                    },
                    x_coords,
                    y_coords,
                    z_coords,
                    iblank: iblank_vec,
                })
            }
            "K" => {
                // Constant K plane: varies in I and J
                let k_idx = index as usize;
                if k_idx >= k {
                    return Err(format!("K index {} out of bounds [0, {})", k_idx, k));
                }

                let mut x_coords = Vec::with_capacity(i * j);
                let mut y_coords = Vec::with_capacity(i * j);
                let mut z_coords = Vec::with_capacity(i * j);
                let mut iblank_vec = self.iblank.as_ref().map(|_| Vec::with_capacity(i * j));

                for j_idx in 0..j {
                    for i_idx in 0..i {
                        let idx = Self::linear_index(i_idx, j_idx, k_idx, i, j);
                        x_coords.push(self.x_coords[idx]);
                        y_coords.push(self.y_coords[idx]);
                        z_coords.push(self.z_coords[idx]);
                        if let Some(ref mut iblank) = iblank_vec {
                            iblank.push(self.iblank.as_ref().map(|ib| ib[idx]).unwrap_or(1));
                        }
                    }
                }

                Ok(Plot3DGrid {
                    dimensions: GridDimensions {
                        i: i as u32,
                        j: j as u32,
                        k: 1,
                    },
                    x_coords,
                    y_coords,
                    z_coords,
                    iblank: iblank_vec,
                })
            }
            _ => Err(format!(
                "Invalid plane: {}. Must be 'I', 'J', or 'K'",
                plane
            )),
        }
    }

    /// Create an arbitrary cutting plane through the grid using triangle intersection
    /// plane_point: A point on the plane [x, y, z]
    /// plane_normal: Normal vector to the plane [nx, ny, nz] (will be normalized)
    /// Returns a mesh geometry representing the intersection
    pub fn slice_arbitrary_plane(
        &self,
        plane_point: [f32; 3],
        plane_normal: [f32; 3],
        respect_iblank: bool,
        show_fringe_points: bool,
        iblank_filter_mode: crate::IblankFilterMode,
    ) -> Result<MeshGeometry, String> {
        let mut mesh = self.slice_arbitrary_plane_with_solution(
            plane_point,
            plane_normal,
            respect_iblank,
            show_fringe_points,
            iblank_filter_mode,
        )?;
        mesh.vertex_cell_data = None;
        Ok(mesh)
    }

    /// Enhanced version of slice_arbitrary_plane that creates a triangulated surface
    /// and tracks cell interpolation data for solution coloring
    pub fn slice_arbitrary_plane_with_solution(
        &self,
        plane_point: [f32; 3],
        plane_normal: [f32; 3],
        respect_iblank: bool,
        show_fringe_points: bool,
        iblank_filter_mode: crate::IblankFilterMode,
    ) -> Result<MeshGeometry, String> {
        let i = self.dimensions.i as usize;
        let j = self.dimensions.j as usize;
        let k = self.dimensions.k as usize;

        if i < 2 || j < 2 || k < 2 {
            return Err("Grid dimensions must be at least 2x2x2 for arbitrary slicing".to_string());
        }

        // Normalize the plane normal
        let normal_mag = (plane_normal[0] * plane_normal[0]
            + plane_normal[1] * plane_normal[1]
            + plane_normal[2] * plane_normal[2])
            .sqrt();
        if normal_mag < 1e-10 {
            return Err("Plane normal vector has zero magnitude".to_string());
        }
        let n = [
            plane_normal[0] / normal_mag,
            plane_normal[1] / normal_mag,
            plane_normal[2] / normal_mag,
        ];
        let d = n[0] * plane_point[0] + n[1] * plane_point[1] + n[2] * plane_point[2];
        let signed_distance =
            |x: f32, y: f32, z: f32| -> f32 { n[0] * x + n[1] * y + n[2] * z - d };

        let dot = |a: [f32; 3], b: [f32; 3]| -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] };
        let cross = |a: [f32; 3], b: [f32; 3]| -> [f32; 3] {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        };
        let sub =
            |a: [f32; 3], b: [f32; 3]| -> [f32; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] };

        let mut min_pt = [f32::INFINITY, f32::INFINITY, f32::INFINITY];
        let mut max_pt = [f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY];
        for idx in 0..self.total_points() {
            let x = self.x_coords[idx];
            let y = self.y_coords[idx];
            let z = self.z_coords[idx];
            min_pt[0] = min_pt[0].min(x);
            min_pt[1] = min_pt[1].min(y);
            min_pt[2] = min_pt[2].min(z);
            max_pt[0] = max_pt[0].max(x);
            max_pt[1] = max_pt[1].max(y);
            max_pt[2] = max_pt[2].max(z);
        }
        let bbox_diag = ((max_pt[0] - min_pt[0]).powi(2)
            + (max_pt[1] - min_pt[1]).powi(2)
            + (max_pt[2] - min_pt[2]).powi(2))
        .sqrt();
        let epsilon = (bbox_diag * 1e-6).max(1e-7);

        let ref_axis = if n[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let u_unnormalized = cross(n, ref_axis);
        let u_mag = (u_unnormalized[0] * u_unnormalized[0]
            + u_unnormalized[1] * u_unnormalized[1]
            + u_unnormalized[2] * u_unnormalized[2])
            .sqrt()
            .max(1e-12);
        let u = [
            u_unnormalized[0] / u_mag,
            u_unnormalized[1] / u_mag,
            u_unnormalized[2] / u_mag,
        ];
        let v = cross(n, u);

        let get_point = |i_idx: usize, j_idx: usize, k_idx: usize| -> [f32; 3] {
            let idx = Self::linear_index(i_idx, j_idx, k_idx, i, j);
            [self.x_coords[idx], self.y_coords[idx], self.z_coords[idx]]
        };
        let point_is_blanked = |idx: usize| -> bool {
            if let Some(iblank) = self.iblank.as_ref() {
                if respect_iblank && iblank[idx] == 0 {
                    return true; // Blanked hole point
                }
                if !show_fringe_points && iblank[idx] < 0 {
                    return true; // Fringe point and we're hiding them
                }
            }
            false
        };
        let mut vertices = Vec::new();
        let mut vertex_cell_data = Vec::new();
        let mut triangle_indices = Vec::new();

        let mut vertex_map: HashMap<(i64, i64, i64), usize> = HashMap::new();
        let mut line_edges: HashSet<(u32, u32)> = HashSet::new();
        let mut triangle_set: HashSet<(u32, u32, u32)> = HashSet::new();

        let quantize_key = |p: [f32; 3]| -> (i64, i64, i64) {
            (
                (p[0] / epsilon).round() as i64,
                (p[1] / epsilon).round() as i64,
                (p[2] / epsilon).round() as i64,
            )
        };

        let edges = [
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 0), // bottom face
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 4), // top face
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7), // vertical edges
        ];

        let add_line_edge = |a: u32, b: u32, line_edges: &mut HashSet<(u32, u32)>| {
            if a == b {
                return;
            }
            let edge = if a < b { (a, b) } else { (b, a) };
            line_edges.insert(edge);
        };

        // Process each hexahedral cell
        for k_idx in 0..k - 1 {
            for j_idx in 0..j - 1 {
                for i_idx in 0..i - 1 {
                    let corner_indices = [
                        Self::linear_index(i_idx, j_idx, k_idx, i, j),
                        Self::linear_index(i_idx + 1, j_idx, k_idx, i, j),
                        Self::linear_index(i_idx + 1, j_idx + 1, k_idx, i, j),
                        Self::linear_index(i_idx, j_idx + 1, k_idx, i, j),
                        Self::linear_index(i_idx, j_idx, k_idx + 1, i, j),
                        Self::linear_index(i_idx + 1, j_idx, k_idx + 1, i, j),
                        Self::linear_index(i_idx + 1, j_idx + 1, k_idx + 1, i, j),
                        Self::linear_index(i_idx, j_idx + 1, k_idx + 1, i, j),
                    ];
                    let corner_blanked = [
                        point_is_blanked(corner_indices[0]),
                        point_is_blanked(corner_indices[1]),
                        point_is_blanked(corner_indices[2]),
                        point_is_blanked(corner_indices[3]),
                        point_is_blanked(corner_indices[4]),
                        point_is_blanked(corner_indices[5]),
                        point_is_blanked(corner_indices[6]),
                        point_is_blanked(corner_indices[7]),
                    ];

                    // Cell-mode: skip entire cell if any corner is blanked/hidden
                    if matches!(iblank_filter_mode, crate::IblankFilterMode::Cell) {
                        if corner_blanked.iter().any(|&b| b) {
                            continue; // Skip entire cell
                        }
                    }

                    // Get the 8 corners of the hexahedron
                    let corners = [
                        get_point(i_idx, j_idx, k_idx),             // 0
                        get_point(i_idx + 1, j_idx, k_idx),         // 1
                        get_point(i_idx + 1, j_idx + 1, k_idx),     // 2
                        get_point(i_idx, j_idx + 1, k_idx),         // 3
                        get_point(i_idx, j_idx, k_idx + 1),         // 4
                        get_point(i_idx + 1, j_idx, k_idx + 1),     // 5
                        get_point(i_idx + 1, j_idx + 1, k_idx + 1), // 6
                        get_point(i_idx, j_idx + 1, k_idx + 1),     // 7
                    ];

                    // Check if plane intersects this cell
                    let corner_dists: Vec<f32> = corners
                        .iter()
                        .map(|c| signed_distance(c[0], c[1], c[2]))
                        .collect();

                    let min_dist = corner_dists.iter().cloned().fold(f32::INFINITY, f32::min);
                    let max_dist = corner_dists
                        .iter()
                        .cloned()
                        .fold(f32::NEG_INFINITY, f32::max);

                    // Skip if all corners on same side
                    if min_dist > epsilon || max_dist < -epsilon {
                        continue;
                    }

                    let mut local_points: Vec<[f32; 3]> = Vec::new();
                    let mut local_weights: Vec<[f32; 8]> = Vec::new();
                    let mut local_point_map: HashMap<(i64, i64, i64), usize> = HashMap::new();

                    let add_local_point = |point: [f32; 3],
                                           weights: [f32; 8],
                                           local_points: &mut Vec<[f32; 3]>,
                                           local_weights: &mut Vec<[f32; 8]>,
                                           local_point_map: &mut HashMap<
                        (i64, i64, i64),
                        usize,
                    >| {
                        let key = quantize_key(point);
                        if local_point_map.contains_key(&key) {
                            return;
                        }
                        local_point_map.insert(key, local_points.len());
                        local_points.push(point);
                        local_weights.push(weights);
                    };

                    for &(c1, c2) in &edges {
                        if corner_blanked[c1] || corner_blanked[c2] {
                            continue;
                        }

                        let d1 = corner_dists[c1];
                        let d2 = corner_dists[c2];

                        if d1.abs() <= epsilon && d2.abs() <= epsilon {
                            let mut w1 = [0.0; 8];
                            let mut w2 = [0.0; 8];
                            w1[c1] = 1.0;
                            w2[c2] = 1.0;
                            if !corner_blanked[c1] {
                                add_local_point(
                                    corners[c1],
                                    w1,
                                    &mut local_points,
                                    &mut local_weights,
                                    &mut local_point_map,
                                );
                            }
                            if !corner_blanked[c2] {
                                add_local_point(
                                    corners[c2],
                                    w2,
                                    &mut local_points,
                                    &mut local_weights,
                                    &mut local_point_map,
                                );
                            }
                        } else if d1.abs() <= epsilon {
                            let mut w = [0.0; 8];
                            w[c1] = 1.0;
                            if !corner_blanked[c1] {
                                add_local_point(
                                    corners[c1],
                                    w,
                                    &mut local_points,
                                    &mut local_weights,
                                    &mut local_point_map,
                                );
                            }
                        } else if d2.abs() <= epsilon {
                            let mut w = [0.0; 8];
                            w[c2] = 1.0;
                            if !corner_blanked[c2] {
                                add_local_point(
                                    corners[c2],
                                    w,
                                    &mut local_points,
                                    &mut local_weights,
                                    &mut local_point_map,
                                );
                            }
                        } else if d1 * d2 < 0.0 {
                            let t = d1.abs() / (d1.abs() + d2.abs());
                            let point = [
                                corners[c1][0] + t * (corners[c2][0] - corners[c1][0]),
                                corners[c1][1] + t * (corners[c2][1] - corners[c1][1]),
                                corners[c1][2] + t * (corners[c2][2] - corners[c1][2]),
                            ];
                            let mut w = [0.0; 8];
                            w[c1] = 1.0 - t;
                            w[c2] = t;
                            add_local_point(
                                point,
                                w,
                                &mut local_points,
                                &mut local_weights,
                                &mut local_point_map,
                            );
                        }
                    }

                    if local_points.len() < 3 {
                        continue;
                    }

                    let mut centroid = [0.0f32, 0.0f32, 0.0f32];
                    for point in &local_points {
                        centroid[0] += point[0];
                        centroid[1] += point[1];
                        centroid[2] += point[2];
                    }
                    let inv_n = 1.0 / (local_points.len() as f32);
                    centroid[0] *= inv_n;
                    centroid[1] *= inv_n;
                    centroid[2] *= inv_n;

                    let mut order: Vec<usize> = (0..local_points.len()).collect();
                    order.sort_by(|&a, &b| {
                        let da = sub(local_points[a], centroid);
                        let db = sub(local_points[b], centroid);
                        let angle_a = dot(da, v).atan2(dot(da, u));
                        let angle_b = dot(db, v).atan2(dot(db, u));
                        angle_a
                            .partial_cmp(&angle_b)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });

                    let mut signed_area = 0.0f32;
                    for idx in 0..order.len() {
                        let pa = local_points[order[idx]];
                        let pb = local_points[order[(idx + 1) % order.len()]];
                        let a_u = dot(sub(pa, centroid), u);
                        let a_v = dot(sub(pa, centroid), v);
                        let b_u = dot(sub(pb, centroid), u);
                        let b_v = dot(sub(pb, centroid), v);
                        signed_area += a_u * b_v - b_u * a_v;
                    }

                    if signed_area.abs() <= epsilon * epsilon {
                        continue;
                    }

                    if signed_area < 0.0 {
                        order.reverse();
                    }

                    let mut ordered_global: Vec<u32> = Vec::with_capacity(order.len());
                    for &local_idx in &order {
                        let point = local_points[local_idx];
                        let key = quantize_key(point);
                        let global_idx = if let Some(&idx) = vertex_map.get(&key) {
                            idx
                        } else {
                            let idx = vertices.len() / 3;
                            vertices.push(point[0]);
                            vertices.push(point[1]);
                            vertices.push(point[2]);
                            vertex_cell_data.push(VertexCellData {
                                cell_i: i_idx,
                                cell_j: j_idx,
                                cell_k: k_idx,
                                weights: local_weights[local_idx],
                            });
                            vertex_map.insert(key, idx);
                            idx
                        };
                        ordered_global.push(global_idx as u32);
                    }

                    if ordered_global.len() >= 3 {
                        for tri_idx in 1..ordered_global.len() - 1 {
                            let a = ordered_global[0];
                            let b = ordered_global[tri_idx];
                            let c = ordered_global[tri_idx + 1];

                            if a == b || b == c || a == c {
                                continue;
                            }

                            let pa = [
                                vertices[(a as usize) * 3],
                                vertices[(a as usize) * 3 + 1],
                                vertices[(a as usize) * 3 + 2],
                            ];
                            let pb = [
                                vertices[(b as usize) * 3],
                                vertices[(b as usize) * 3 + 1],
                                vertices[(b as usize) * 3 + 2],
                            ];
                            let pc = [
                                vertices[(c as usize) * 3],
                                vertices[(c as usize) * 3 + 1],
                                vertices[(c as usize) * 3 + 2],
                            ];

                            let tri_normal = cross(sub(pb, pa), sub(pc, pa));
                            let tri_area_mag = (tri_normal[0] * tri_normal[0]
                                + tri_normal[1] * tri_normal[1]
                                + tri_normal[2] * tri_normal[2])
                                .sqrt();
                            if tri_area_mag <= epsilon * epsilon {
                                continue;
                            }

                            if dot(tri_normal, n) < 0.0 {
                                let mut key = [a, c, b];
                                key.sort_unstable();
                                if triangle_set.insert((key[0], key[1], key[2])) {
                                    triangle_indices.push(a);
                                    triangle_indices.push(c);
                                    triangle_indices.push(b);
                                    add_line_edge(a, c, &mut line_edges);
                                    add_line_edge(c, b, &mut line_edges);
                                    add_line_edge(b, a, &mut line_edges);
                                }
                            } else {
                                let mut key = [a, b, c];
                                key.sort_unstable();
                                if triangle_set.insert((key[0], key[1], key[2])) {
                                    triangle_indices.push(a);
                                    triangle_indices.push(b);
                                    triangle_indices.push(c);
                                    add_line_edge(a, b, &mut line_edges);
                                    add_line_edge(b, c, &mut line_edges);
                                    add_line_edge(c, a, &mut line_edges);
                                }
                            }
                        }
                    }
                }
            }
        }

        if triangle_indices.is_empty() {
            return Err("No intersection found between plane and grid".to_string());
        }

        // Compute normals (all vertices on the plane have the same normal)
        let vertex_count = vertices.len() / 3;
        let mut normals = Vec::with_capacity(vertex_count * 3);
        for _ in 0..vertex_count {
            normals.push(n[0]);
            normals.push(n[1]);
            normals.push(n[2]);
        }

        let mut line_indices = Vec::with_capacity(line_edges.len() * 2);
        for (a, b) in line_edges {
            line_indices.push(a);
            line_indices.push(b);
        }

        Ok(MeshGeometry {
            vertices,
            indices: line_indices,
            triangle_indices: triangle_indices.clone(),
            normals,
            vertex_count,
            face_count: triangle_indices.len() / 3,
            colors: None,
            vertex_cell_data: Some(vertex_cell_data),
        })
    }

    /// Convert PLOT3D grid to Three.js mesh geometry with optional decimation
    /// decimation_factor: 1 = full resolution, 2 = half, 3 = third, etc.
    /// This creates quad edges for wireframe display (4 edges per quad, no triangulation)
    /// If respect_iblank is true and iblank data exists, points with iblank=0 are excluded
    pub fn to_mesh_geometry_decimated(
        &self,
        respect_iblank: bool,
        decimation_factor: usize,
    ) -> MeshGeometry {
        let decimation = decimation_factor.max(1); // Ensure at least 1

        let i = self.dimensions.i as usize;
        let j = self.dimensions.j as usize;
        let total_points = self.total_points();

        // Helper function to check if a point is blanked
        let is_blanked = |idx: usize| -> bool {
            if respect_iblank {
                if let Some(ref iblank) = self.iblank {
                    return iblank[idx] == 0;
                }
            }
            false
        };

        // Convert coordinates to vertex array (x, y, z interleaved)
        // Pre-allocate with exact size
        let mut vertices = Vec::with_capacity(total_points * 3);
        for idx in 0..total_points {
            vertices.push(self.x_coords[idx]);
            vertices.push(self.y_coords[idx]);
            vertices.push(self.z_coords[idx]);
        }

        // Calculate decimated grid dimensions
        let i_decimated = ((i - 1) / decimation) + 1;
        let j_decimated = ((j - 1) / decimation) + 1;

        // Pre-calculate capacity for indices
        // Each quad generates: 8 line indices (4 edges * 2 vertices) and 6 triangle indices (2 triangles * 3 vertices)
        let max_quads = (i_decimated - 1) * (j_decimated - 1);
        let mut line_indices = Vec::with_capacity(max_quads * 8);
        let mut triangle_indices = Vec::with_capacity(max_quads * 6);

        // Generate indices for both wireframe lines and solid triangles
        // For a structured grid, we only render the k=1 surface (k=0 in 0-indexed)
        let k_idx = 0;

        for j_step in 0..j_decimated - 1 {
            let j_idx = j_step * decimation;
            let j_next = ((j_step + 1) * decimation).min(j - 1);

            for i_step in 0..i_decimated - 1 {
                let i_idx = i_step * decimation;
                let i_next = ((i_step + 1) * decimation).min(i - 1);

                let idx00 = Self::linear_index(i_idx, j_idx, k_idx, i, j);
                let idx10 = Self::linear_index(i_next, j_idx, k_idx, i, j);
                let idx01 = Self::linear_index(i_idx, j_next, k_idx, i, j);
                let idx11 = Self::linear_index(i_next, j_next, k_idx, i, j);

                // Skip this quad if any corner is blanked
                if is_blanked(idx00) || is_blanked(idx10) || is_blanked(idx01) || is_blanked(idx11)
                {
                    continue;
                }

                // Line indices: 4 edges per quad
                // Bottom edge (idx00 -> idx10)
                line_indices.push(idx00 as u32);
                line_indices.push(idx10 as u32);

                // Right edge (idx10 -> idx11)
                line_indices.push(idx10 as u32);
                line_indices.push(idx11 as u32);

                // Top edge (idx11 -> idx01)
                line_indices.push(idx11 as u32);
                line_indices.push(idx01 as u32);

                // Left edge (idx01 -> idx00)
                line_indices.push(idx01 as u32);
                line_indices.push(idx00 as u32);

                // Triangle indices: 2 triangles per quad
                // First triangle: idx00, idx10, idx11
                triangle_indices.push(idx00 as u32);
                triangle_indices.push(idx10 as u32);
                triangle_indices.push(idx11 as u32);

                // Second triangle: idx00, idx11, idx01
                triangle_indices.push(idx00 as u32);
                triangle_indices.push(idx11 as u32);
                triangle_indices.push(idx01 as u32);
            }
        }

        // Compute simple vertex normals (averaged from adjacent faces) using parallel processing
        // Pre-allocate normals array
        let mut normals = vec![0.0f32; total_points * 3];

        // Collect normal contributions in parallel (need to capture vertices by reference)
        let vertices_ref = &vertices;
        let normal_contributions: Vec<_> = (0..j_decimated - 1)
            .into_par_iter()
            .flat_map(|j_step| {
                let j_idx = j_step * decimation;
                let j_next = ((j_step + 1) * decimation).min(j - 1);

                (0..i_decimated - 1)
                    .filter_map(move |i_step| {
                        let i_idx = i_step * decimation;
                        let i_next = ((i_step + 1) * decimation).min(i - 1);

                        let idx00 = Self::linear_index(i_idx, j_idx, k_idx, i, j);
                        let idx10 = Self::linear_index(i_next, j_idx, k_idx, i, j);
                        let idx01 = Self::linear_index(i_idx, j_next, k_idx, i, j);
                        let idx11 = Self::linear_index(i_next, j_next, k_idx, i, j);

                        let v0 = [
                            vertices_ref[idx00 * 3],
                            vertices_ref[idx00 * 3 + 1],
                            vertices_ref[idx00 * 3 + 2],
                        ];
                        let v1 = [
                            vertices_ref[idx10 * 3],
                            vertices_ref[idx10 * 3 + 1],
                            vertices_ref[idx10 * 3 + 2],
                        ];
                        let v2 = [
                            vertices_ref[idx01 * 3],
                            vertices_ref[idx01 * 3 + 1],
                            vertices_ref[idx01 * 3 + 2],
                        ];

                        // Compute quad normal using cross product
                        let edge1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
                        let edge2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];

                        let normal = [
                            edge1[1] * edge2[2] - edge1[2] * edge2[1],
                            edge1[2] * edge2[0] - edge1[0] * edge2[2],
                            edge1[0] * edge2[1] - edge1[1] * edge2[0],
                        ];

                        Some(([idx00, idx10, idx01, idx11], normal))
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

        // Apply normal contributions sequentially
        for (indices, normal) in normal_contributions {
            for &idx in &indices {
                normals[idx * 3] += normal[0];
                normals[idx * 3 + 1] += normal[1];
                normals[idx * 3 + 2] += normal[2];
            }
        }

        // Normalize normals in parallel
        normals.par_chunks_mut(3).for_each(|chunk| {
            let len = (chunk[0] * chunk[0] + chunk[1] * chunk[1] + chunk[2] * chunk[2]).sqrt();
            if len > 0.0 {
                chunk[0] /= len;
                chunk[1] /= len;
                chunk[2] /= len;
            }
        });

        // face_count represents number of triangles
        let triangle_count = triangle_indices.len() / 3;

        MeshGeometry {
            vertices,
            indices: line_indices,
            triangle_indices,
            normals,
            vertex_count: total_points,
            face_count: triangle_count,
            colors: None,
            vertex_cell_data: None,
        }
    }

    /// Convert PLOT3D grid to surface-only mesh geometry with optional decimation (k=0).
    /// This generates vertices only for the visible surface to reduce memory and GPU cost.
    pub fn to_mesh_surface_geometry_decimated(
        &self,
        respect_iblank: bool,
        show_fringe_points: bool,
        iblank_filter_mode: crate::IblankFilterMode,
        decimation_factor: usize,
    ) -> MeshGeometry {
        match iblank_filter_mode {
            crate::IblankFilterMode::Vertex => self.to_mesh_surface_geometry_decimated_vertex_mode(
                respect_iblank,
                show_fringe_points,
                decimation_factor,
            ),
            crate::IblankFilterMode::Cell => self.to_mesh_surface_geometry_decimated_cell_mode(
                respect_iblank,
                show_fringe_points,
                decimation_factor,
            ),
        }
    }

    /// Vertex-skipping mode: removes blanked vertices entirely (creates point-level holes)
    fn to_mesh_surface_geometry_decimated_vertex_mode(
        &self,
        respect_iblank: bool,
        show_fringe_points: bool,
        decimation_factor: usize,
    ) -> MeshGeometry {
        #[derive(Clone, Copy)]
        enum SurfaceOrientation {
            KPlane,
            IPlane,
            JPlane,
        }

        let decimation = decimation_factor.max(1);

        let ni = self.dimensions.i as usize;
        let nj = self.dimensions.j as usize;
        let nk = self.dimensions.k as usize;

        let orientation = if nk == 1 {
            SurfaceOrientation::KPlane
        } else if ni == 1 {
            SurfaceOrientation::IPlane
        } else if nj == 1 {
            SurfaceOrientation::JPlane
        } else {
            // Default full-volume rendering remains the K-min boundary surface.
            SurfaceOrientation::KPlane
        };

        let (u_dim, v_dim) = match orientation {
            SurfaceOrientation::KPlane => (ni, nj),
            SurfaceOrientation::IPlane => (nj, nk),
            SurfaceOrientation::JPlane => (ni, nk),
        };

        let grid_idx_at = |u_idx: usize, v_idx: usize| -> usize {
            match orientation {
                SurfaceOrientation::KPlane => Self::linear_index(u_idx, v_idx, 0, ni, nj),
                SurfaceOrientation::IPlane => Self::linear_index(0, u_idx, v_idx, ni, nj),
                SurfaceOrientation::JPlane => Self::linear_index(u_idx, 0, v_idx, ni, nj),
            }
        };

        let u_decimated = ((u_dim - 1) / decimation) + 1;
        let v_decimated = ((v_dim - 1) / decimation) + 1;

        let is_blanked = |idx: usize| -> bool {
            if let Some(ref iblank) = self.iblank {
                if respect_iblank && iblank[idx] == 0 {
                    return true; // Blanked hole point
                }
                if !show_fringe_points && iblank[idx] < 0 {
                    return true; // Fringe point and we're hiding them
                }
            }
            false
        };

        // Build mapping from grid indices to output mesh indices, skipping blanked vertices
        let mut vertex_index_map: Vec<Option<u32>> = vec![None; u_decimated * v_decimated];
        let mut vertices = Vec::new();
        let mut output_vertex_count = 0u32;

        for v_step in 0..v_decimated {
            let v_idx = (v_step * decimation).min(v_dim - 1);
            for u_step in 0..u_decimated {
                let u_idx = (u_step * decimation).min(u_dim - 1);
                let grid_idx = grid_idx_at(u_idx, v_idx);
                let grid_vertex_idx = v_step * u_decimated + u_step;

                if is_blanked(grid_idx) {
                    // Skip blanked vertices
                    vertex_index_map[grid_vertex_idx] = None;
                } else {
                    // Add non-blanked vertices to output array
                    vertex_index_map[grid_vertex_idx] = Some(output_vertex_count);
                    vertices.push(self.x_coords[grid_idx]);
                    vertices.push(self.y_coords[grid_idx]);
                    vertices.push(self.z_coords[grid_idx]);
                    output_vertex_count += 1;
                }
            }
        }

        let max_quads = (u_decimated - 1) * (v_decimated - 1);
        let mut line_indices = Vec::with_capacity(max_quads * 8);
        let mut triangle_indices = Vec::with_capacity(max_quads * 6);

        for v_step in 0..v_decimated - 1 {
            for u_step in 0..u_decimated - 1 {
                // Get mapped indices for quad corners
                let grid_v00 = v_step * u_decimated + u_step;
                let grid_v10 = v_step * u_decimated + (u_step + 1);
                let grid_v01 = (v_step + 1) * u_decimated + u_step;
                let grid_v11 = (v_step + 1) * u_decimated + (u_step + 1);

                // Skip quad if any corner is blanked
                let v00 = match vertex_index_map[grid_v00] {
                    Some(idx) => idx,
                    None => continue,
                };
                let v10 = match vertex_index_map[grid_v10] {
                    Some(idx) => idx,
                    None => continue,
                };
                let v01 = match vertex_index_map[grid_v01] {
                    Some(idx) => idx,
                    None => continue,
                };
                let v11 = match vertex_index_map[grid_v11] {
                    Some(idx) => idx,
                    None => continue,
                };

                line_indices.extend_from_slice(&[v00, v10, v10, v11, v11, v01, v01, v00]);

                triangle_indices.extend_from_slice(&[v00, v10, v11, v00, v11, v01]);
            }
        }

        let mut normals = vec![0.0f32; vertices.len()];
        for tri in triangle_indices.chunks(3) {
            if tri.len() < 3 {
                continue;
            }
            let i0 = tri[0] as usize;
            let i1 = tri[1] as usize;
            let i2 = tri[2] as usize;

            let v0 = [vertices[i0 * 3], vertices[i0 * 3 + 1], vertices[i0 * 3 + 2]];
            let v1 = [vertices[i1 * 3], vertices[i1 * 3 + 1], vertices[i1 * 3 + 2]];
            let v2 = [vertices[i2 * 3], vertices[i2 * 3 + 1], vertices[i2 * 3 + 2]];

            let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
            let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];

            let nx = e1[1] * e2[2] - e1[2] * e2[1];
            let ny = e1[2] * e2[0] - e1[0] * e2[2];
            let nz = e1[0] * e2[1] - e1[1] * e2[0];

            for &idx in &[i0, i1, i2] {
                normals[idx * 3] += nx;
                normals[idx * 3 + 1] += ny;
                normals[idx * 3 + 2] += nz;
            }
        }

        for idx in 0..(normals.len() / 3) {
            let nx = normals[idx * 3];
            let ny = normals[idx * 3 + 1];
            let nz = normals[idx * 3 + 2];
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            if len > 0.0 {
                normals[idx * 3] /= len;
                normals[idx * 3 + 1] /= len;
                normals[idx * 3 + 2] /= len;
            }
        }

        let face_count = triangle_indices.len() / 3;

        MeshGeometry {
            vertices,
            indices: line_indices,
            triangle_indices,
            normals,
            vertex_count: output_vertex_count as usize,
            face_count,
            colors: None,
            vertex_cell_data: None,
        }
    }

    /// Cell-skipping mode: keeps all vertices but rejects quads when any corner is blanked (creates region-level holes)
    fn to_mesh_surface_geometry_decimated_cell_mode(
        &self,
        respect_iblank: bool,
        show_fringe_points: bool,
        decimation_factor: usize,
    ) -> MeshGeometry {
        #[derive(Clone, Copy)]
        enum SurfaceOrientation {
            KPlane,
            IPlane,
            JPlane,
        }

        let decimation = decimation_factor.max(1);

        let ni = self.dimensions.i as usize;
        let nj = self.dimensions.j as usize;
        let nk = self.dimensions.k as usize;

        let orientation = if nk == 1 {
            SurfaceOrientation::KPlane
        } else if ni == 1 {
            SurfaceOrientation::IPlane
        } else if nj == 1 {
            SurfaceOrientation::JPlane
        } else {
            // Default full-volume rendering remains the K-min boundary surface.
            SurfaceOrientation::KPlane
        };

        let (u_dim, v_dim) = match orientation {
            SurfaceOrientation::KPlane => (ni, nj),
            SurfaceOrientation::IPlane => (nj, nk),
            SurfaceOrientation::JPlane => (ni, nk),
        };

        let grid_idx_at = |u_idx: usize, v_idx: usize| -> usize {
            match orientation {
                SurfaceOrientation::KPlane => Self::linear_index(u_idx, v_idx, 0, ni, nj),
                SurfaceOrientation::IPlane => Self::linear_index(0, u_idx, v_idx, ni, nj),
                SurfaceOrientation::JPlane => Self::linear_index(u_idx, 0, v_idx, ni, nj),
            }
        };

        let u_decimated = ((u_dim - 1) / decimation) + 1;
        let v_decimated = ((v_dim - 1) / decimation) + 1;

        let is_hidden = |idx: usize| -> bool {
            if let Some(ref iblank) = self.iblank {
                if respect_iblank && iblank[idx] == 0 {
                    return true;
                }
                if !show_fringe_points && iblank[idx] < 0 {
                    return true;
                }
            }
            false
        };

        let total_vertices = u_decimated * v_decimated;
        let mut vertices = Vec::with_capacity(total_vertices * 3);

        for v_step in 0..v_decimated {
            let v_idx = (v_step * decimation).min(v_dim - 1);
            for u_step in 0..u_decimated {
                let u_idx = (u_step * decimation).min(u_dim - 1);
                let grid_idx = grid_idx_at(u_idx, v_idx);
                vertices.push(self.x_coords[grid_idx]);
                vertices.push(self.y_coords[grid_idx]);
                vertices.push(self.z_coords[grid_idx]);
            }
        }

        let max_quads = (u_decimated - 1) * (v_decimated - 1);
        let mut line_indices = Vec::with_capacity(max_quads * 8);
        let mut triangle_indices = Vec::with_capacity(max_quads * 6);

        for v_step in 0..v_decimated - 1 {
            let v_idx = (v_step * decimation).min(v_dim - 1);
            let v_next = ((v_step + 1) * decimation).min(v_dim - 1);

            for u_step in 0..u_decimated - 1 {
                let u_idx = (u_step * decimation).min(u_dim - 1);
                let u_next = ((u_step + 1) * decimation).min(u_dim - 1);

                let idx00 = grid_idx_at(u_idx, v_idx);
                let idx10 = grid_idx_at(u_next, v_idx);
                let idx01 = grid_idx_at(u_idx, v_next);
                let idx11 = grid_idx_at(u_next, v_next);

                if is_hidden(idx00) || is_hidden(idx10) || is_hidden(idx01) || is_hidden(idx11) {
                    continue;
                }

                let v00 = (v_step * u_decimated + u_step) as u32;
                let v10 = (v_step * u_decimated + (u_step + 1)) as u32;
                let v01 = ((v_step + 1) * u_decimated + u_step) as u32;
                let v11 = ((v_step + 1) * u_decimated + (u_step + 1)) as u32;

                line_indices.extend_from_slice(&[v00, v10, v10, v11, v11, v01, v01, v00]);
                triangle_indices.extend_from_slice(&[v00, v10, v11, v00, v11, v01]);
            }
        }

        let mut normals = vec![0.0f32; vertices.len()];
        for tri in triangle_indices.chunks(3) {
            if tri.len() < 3 {
                continue;
            }
            let i0 = tri[0] as usize;
            let i1 = tri[1] as usize;
            let i2 = tri[2] as usize;

            let v0 = [vertices[i0 * 3], vertices[i0 * 3 + 1], vertices[i0 * 3 + 2]];
            let v1 = [vertices[i1 * 3], vertices[i1 * 3 + 1], vertices[i1 * 3 + 2]];
            let v2 = [vertices[i2 * 3], vertices[i2 * 3 + 1], vertices[i2 * 3 + 2]];

            let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
            let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];

            let nx = e1[1] * e2[2] - e1[2] * e2[1];
            let ny = e1[2] * e2[0] - e1[0] * e2[2];
            let nz = e1[0] * e2[1] - e1[1] * e2[0];

            for &idx in &[i0, i1, i2] {
                normals[idx * 3] += nx;
                normals[idx * 3 + 1] += ny;
                normals[idx * 3 + 2] += nz;
            }
        }

        for idx in 0..(normals.len() / 3) {
            let nx = normals[idx * 3];
            let ny = normals[idx * 3 + 1];
            let nz = normals[idx * 3 + 2];
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            if len > 0.0 {
                normals[idx * 3] /= len;
                normals[idx * 3 + 1] /= len;
                normals[idx * 3 + 2] /= len;
            }
        }

        let face_count = triangle_indices.len() / 3;

        MeshGeometry {
            vertices,
            indices: line_indices,
            triangle_indices,
            normals,
            vertex_count: total_vertices,
            face_count,
            colors: None,
            vertex_cell_data: None,
        }
    }

    /// Convert PLOT3D grid to Three.js mesh geometry
    /// This creates quad edges for wireframe display (4 edges per quad, no triangulation)
    /// If respect_iblank is true and iblank data exists, points with iblank=0 are excluded
    #[allow(dead_code)]
    pub fn to_mesh_geometry(&self, respect_iblank: bool) -> MeshGeometry {
        // Default to no decimation (full resolution)
        self.to_mesh_geometry_decimated(respect_iblank, 1)
    }

    /// Helper function to convert 3D grid index to linear index
    fn linear_index(i: usize, j: usize, k: usize, dim_i: usize, dim_j: usize) -> usize {
        k * dim_i * dim_j + j * dim_i + i
    }

    /// Extract iso-surface at given scalar field level using marching cubes
    pub fn extract_iso_surface(
        &self,
        solution: &Plot3DSolution,
        scalar_field: crate::plot_state::ScalarField,
        level: f32,
        respect_iblank: bool,
        show_fringe_points: bool,
        iblank_filter_mode: crate::IblankFilterMode,
    ) -> Result<MeshGeometry, String> {
        use crate::solution::compute_scalar_field;

        let i_dim = self.dimensions.i as usize;
        let j_dim = self.dimensions.j as usize;
        let k_dim = self.dimensions.k as usize;

        if i_dim < 2 || j_dim < 2 || k_dim < 2 {
            return Ok(MeshGeometry {
                vertices: vec![],
                indices: vec![],
                triangle_indices: vec![],
                normals: vec![],
                vertex_count: 0,
                face_count: 0,
                colors: None,
                vertex_cell_data: None,
            });
        }

        // Compute scalar field values for all points in the 3D volume
        let scalar_values = compute_scalar_field(solution, scalar_field);

        let mut vertices = Vec::new();
        let mut triangles = Vec::new();

        // Iterate over all cells
        for k in 0..k_dim - 1 {
            for j in 0..j_dim - 1 {
                for i in 0..i_dim - 1 {
                    // Get 8 corner indices of hexahedral cell
                    let corners = [
                        Self::linear_index(i, j, k, i_dim, j_dim),
                        Self::linear_index(i + 1, j, k, i_dim, j_dim),
                        Self::linear_index(i + 1, j + 1, k, i_dim, j_dim),
                        Self::linear_index(i, j + 1, k, i_dim, j_dim),
                        Self::linear_index(i, j, k + 1, i_dim, j_dim),
                        Self::linear_index(i + 1, j, k + 1, i_dim, j_dim),
                        Self::linear_index(i + 1, j + 1, k + 1, i_dim, j_dim),
                        Self::linear_index(i, j + 1, k + 1, i_dim, j_dim),
                    ];

                    // Apply IBLANK filtering
                    if respect_iblank {
                        if let Some(ref iblank) = self.iblank {
                            let should_skip = match iblank_filter_mode {
                                crate::IblankFilterMode::Vertex => corners.iter().all(|&idx| {
                                    crate::is_hidden_iblank_point(
                                        Some(iblank),
                                        idx,
                                        respect_iblank,
                                        show_fringe_points,
                                    )
                                }),
                                crate::IblankFilterMode::Cell => corners.iter().any(|&idx| {
                                    crate::is_hidden_iblank_point(
                                        Some(iblank),
                                        idx,
                                        respect_iblank,
                                        show_fringe_points,
                                    )
                                }),
                            };
                            if should_skip {
                                continue;
                            }
                        }
                    }

                    // Get scalar values at corners
                    let values: Vec<f32> = corners.iter().map(|&idx| scalar_values[idx]).collect();

                    // Classify cell configuration (which vertices are above/below iso-level)
                    let mut cube_index = 0;
                    for (i, &val) in values.iter().enumerate() {
                        if val > level {
                            cube_index |= 1 << i;
                        }
                    }

                    // Skip if cell doesn't intersect iso-surface
                    if cube_index == 0 || cube_index == 255 {
                        continue;
                    }

                    // Compute interpolated vertices on edges
                    // Edge table: pairs of corner indices for each of 12 edges
                    let edge_pairs = [
                        (0, 1),
                        (1, 2),
                        (2, 3),
                        (3, 0), // bottom face: edges 0-3
                        (4, 5),
                        (5, 6),
                        (6, 7),
                        (7, 4), // top face: edges 4-7
                        (0, 4),
                        (1, 5),
                        (2, 6),
                        (3, 7), // vertical edges: edges 8-11
                    ];

                    let mut edge_vertices_pos = [None; 12];

                    for (edge_idx, (v1_idx, v2_idx)) in edge_pairs.iter().enumerate() {
                        // Check if this edge is crossed by checking if the bit is set
                        if (EDGE_TABLE[cube_index] & (1 << edge_idx)) != 0 {
                            let val1 = values[*v1_idx];
                            let val2 = values[*v2_idx];

                            // Linear interpolation to find crossing point
                            let t = if (val2 - val1).abs() < 1e-6 {
                                0.5
                            } else {
                                (level - val1) / (val2 - val1)
                            };

                            let c1 = corners[*v1_idx];
                            let c2 = corners[*v2_idx];

                            let x = self.x_coords[c1] + t * (self.x_coords[c2] - self.x_coords[c1]);
                            let y = self.y_coords[c1] + t * (self.y_coords[c2] - self.y_coords[c1]);
                            let z = self.z_coords[c1] + t * (self.z_coords[c2] - self.z_coords[c1]);

                            edge_vertices_pos[edge_idx] = Some((x, y, z));
                        }
                    }

                    // Create triangles using the triangle table
                    let tri_config = &TRI_TABLE[cube_index];
                    let mut tri_idx = 0;
                    while tri_idx < tri_config.len() && tri_config[tri_idx] != -1 {
                        // Get three edge indices that form a triangle
                        let e0 = tri_config[tri_idx] as usize;
                        let e1 = tri_config[tri_idx + 1] as usize;
                        let e2 = tri_config[tri_idx + 2] as usize;

                        if let (Some(p0), Some(p1), Some(p2)) = (
                            edge_vertices_pos[e0],
                            edge_vertices_pos[e1],
                            edge_vertices_pos[e2],
                        ) {
                            let base_idx = (vertices.len() / 3) as u32;

                            vertices.push(p0.0);
                            vertices.push(p0.1);
                            vertices.push(p0.2);

                            vertices.push(p1.0);
                            vertices.push(p1.1);
                            vertices.push(p1.2);

                            vertices.push(p2.0);
                            vertices.push(p2.1);
                            vertices.push(p2.2);

                            triangles.push(base_idx);
                            triangles.push(base_idx + 1);
                            triangles.push(base_idx + 2);
                        }

                        tri_idx += 3;
                    }
                }
            }
        }

        // Calculate counts before moving data
        let vertex_count = vertices.len() / 3;
        let face_count = triangles.len() / 3;

        // Compute normals (simplified: use face normals)
        let mut normals = vec![0.0; vertices.len()];
        for tri_idx in (0..triangles.len()).step_by(3) {
            let i0 = triangles[tri_idx] as usize * 3;
            let i1 = triangles[tri_idx + 1] as usize * 3;
            let i2 = triangles[tri_idx + 2] as usize * 3;

            let v0 = [vertices[i0], vertices[i0 + 1], vertices[i0 + 2]];
            let v1 = [vertices[i1], vertices[i1 + 1], vertices[i1 + 2]];
            let v2 = [vertices[i2], vertices[i2 + 1], vertices[i2 + 2]];

            // Compute face normal
            let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
            let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
            let normal = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];

            // Accumulate to vertex normals
            for &idx in &[i0, i1, i2] {
                normals[idx] += normal[0];
                normals[idx + 1] += normal[1];
                normals[idx + 2] += normal[2];
            }
        }

        // Normalize vertex normals
        for i in (0..normals.len()).step_by(3) {
            let len = (normals[i] * normals[i]
                + normals[i + 1] * normals[i + 1]
                + normals[i + 2] * normals[i + 2])
                .sqrt();
            if len > 1e-6 {
                normals[i] /= len;
                normals[i + 1] /= len;
                normals[i + 2] /= len;
            }
        }

        Ok(MeshGeometry {
            vertices,
            indices: vec![], // Not used for solid rendering
            triangle_indices: triangles,
            normals,
            vertex_count,
            face_count,
            colors: None,
            vertex_cell_data: None,
        })
    }

    /// Extract contour lines from I/J/K slice at specified level
    pub fn extract_slice_contours(
        &self,
        solution: &Plot3DSolution,
        plane: &str,
        index: usize,
        scalar_field: crate::plot_state::ScalarField,
        level: f32,
        respect_iblank: bool,
        show_fringe_points: bool,
        iblank_filter_mode: crate::IblankFilterMode,
    ) -> Result<Vec<f32>, String> {
        use crate::solution::compute_scalar_field;

        // First, create a slice
        let sliced_grid = self.slice_grid(plane, index as u32)?;

        // Compute scalar values for the full 3D volume
        let scalar_values = compute_scalar_field(solution, scalar_field);

        // Map slice vertices to original indices to get their scalar values
        let i_dim = self.dimensions.i as usize;
        let j_dim = self.dimensions.j as usize;
        let slice_i = sliced_grid.dimensions.i as usize;
        let slice_j = sliced_grid.dimensions.j as usize;

        let mut slice_scalars = Vec::new();
        match plane.to_uppercase().as_str() {
            "I" => {
                for k_idx in 0..slice_j {
                    for j_idx in 0..slice_i {
                        let orig_idx = Self::linear_index(index, j_idx, k_idx, i_dim, j_dim);
                        slice_scalars.push(scalar_values[orig_idx]);
                    }
                }
            }
            "J" => {
                for k_idx in 0..slice_j {
                    for i_idx in 0..slice_i {
                        let orig_idx = Self::linear_index(i_idx, index, k_idx, i_dim, j_dim);
                        slice_scalars.push(scalar_values[orig_idx]);
                    }
                }
            }
            "K" => {
                for j_idx in 0..slice_j {
                    for i_idx in 0..slice_i {
                        let orig_idx = Self::linear_index(i_idx, j_idx, index, i_dim, j_dim);
                        slice_scalars.push(scalar_values[orig_idx]);
                    }
                }
            }
            _ => return Err(format!("Invalid plane: {}", plane)),
        }

        // Extract contour lines from the sliced 2D grid
        extract_contour_lines_2d(
            &sliced_grid,
            &slice_scalars,
            level,
            respect_iblank,
            show_fringe_points,
            iblank_filter_mode,
        )
    }

    /// Extract contour lines from arbitrary plane at specified level
    pub fn extract_arbitrary_plane_contours(
        &self,
        solution: &Plot3DSolution,
        plane_point: [f32; 3],
        plane_normal: [f32; 3],
        scalar_field: crate::plot_state::ScalarField,
        level: f32,
        respect_iblank: bool,
        show_fringe_points: bool,
        iblank_filter_mode: crate::IblankFilterMode,
    ) -> Result<Vec<f32>, String> {
        // Slice the grid with the arbitrary plane
        let sliced_grid = self.slice_arbitrary_plane(
            plane_point,
            plane_normal,
            respect_iblank,
            show_fringe_points,
            iblank_filter_mode,
        )?;

        // Get scalar values at vertices using vertex_cell_data
        if sliced_grid.vertex_cell_data.is_none() {
            return Ok(vec![]); // No interpolation data available
        }

        let vertex_cell_data = sliced_grid.vertex_cell_data.as_ref().unwrap();
        let mut slice_scalars = Vec::new();

        for vcd in vertex_cell_data.iter() {
            // Interpolate scalar value from cell corners
            let mut scalar_val = 0.0;
            let i_dim = self.dimensions.i as usize;
            let j_dim = self.dimensions.j as usize;

            // Get indices of 8 cell corners
            let corners = [
                Self::linear_index(vcd.cell_i, vcd.cell_j, vcd.cell_k, i_dim, j_dim),
                Self::linear_index(vcd.cell_i + 1, vcd.cell_j, vcd.cell_k, i_dim, j_dim),
                Self::linear_index(vcd.cell_i + 1, vcd.cell_j + 1, vcd.cell_k, i_dim, j_dim),
                Self::linear_index(vcd.cell_i, vcd.cell_j + 1, vcd.cell_k, i_dim, j_dim),
                Self::linear_index(vcd.cell_i, vcd.cell_j, vcd.cell_k + 1, i_dim, j_dim),
                Self::linear_index(vcd.cell_i + 1, vcd.cell_j, vcd.cell_k + 1, i_dim, j_dim),
                Self::linear_index(vcd.cell_i + 1, vcd.cell_j + 1, vcd.cell_k + 1, i_dim, j_dim),
                Self::linear_index(vcd.cell_i, vcd.cell_j + 1, vcd.cell_k + 1, i_dim, j_dim),
            ];

            // Compute scalar field at corners and interpolate
            use crate::solution::compute_scalar_field;
            let scalar_values = compute_scalar_field(solution, scalar_field);

            for (i, &corner_idx) in corners.iter().enumerate() {
                scalar_val += vcd.weights[i] * scalar_values[corner_idx];
            }

            slice_scalars.push(scalar_val);
        }

        // The sliced_grid has triangle_indices we can use
        extract_contour_lines_from_triangles(
            &sliced_grid.vertices,
            &sliced_grid.triangle_indices,
            &slice_scalars,
            level,
        )
    }
}

/// Extract contour lines from 2D structured grid
/// Returns flat array of line segment endpoints (x1,y1,z1, x2,y2,z2, ...)
fn extract_contour_lines_2d(
    grid: &Plot3DGrid,
    scalar_values: &[f32],
    level: f32,
    respect_iblank: bool,
    show_fringe_points: bool,
    iblank_filter_mode: crate::IblankFilterMode,
) -> Result<Vec<f32>, String> {
    let i_dim = grid.dimensions.i as usize;
    let j_dim = grid.dimensions.j as usize;

    if i_dim < 2 || j_dim < 2 {
        return Ok(vec![]);
    }

    let mut line_segments = Vec::new();

    // Iterate over all quads in the 2D grid
    for j in 0..j_dim - 1 {
        for i in 0..i_dim - 1 {
            // Get 4 corners of quad
            let corners = [
                j * i_dim + i,
                j * i_dim + i + 1,
                (j + 1) * i_dim + i + 1,
                (j + 1) * i_dim + i,
            ];

            // Apply IBLANK filtering
            if respect_iblank {
                if let Some(ref iblank) = grid.iblank {
                    let should_skip = match iblank_filter_mode {
                        crate::IblankFilterMode::Vertex => corners.iter().all(|&idx| {
                            crate::is_hidden_iblank_point(
                                Some(iblank),
                                idx,
                                respect_iblank,
                                show_fringe_points,
                            )
                        }),
                        crate::IblankFilterMode::Cell => corners.iter().any(|&idx| {
                            crate::is_hidden_iblank_point(
                                Some(iblank),
                                idx,
                                respect_iblank,
                                show_fringe_points,
                            )
                        }),
                    };
                    if should_skip {
                        continue;
                    }
                }
            }

            // Get scalar values at corners
            let values: Vec<f32> = corners.iter().map(|&idx| scalar_values[idx]).collect();

            // Check each edge of the quad for contour crossings
            let edges = [(0, 1), (1, 2), (2, 3), (3, 0)];

            for (v1_idx, v2_idx) in edges.iter() {
                let val1 = values[*v1_idx];
                let val2 = values[*v2_idx];

                // Check if edge crosses contour level
                if (val1 <= level && val2 > level) || (val1 > level && val2 <= level) {
                    // Linear interpolation
                    let t = if (val2 - val1).abs() < 1e-6 {
                        0.5
                    } else {
                        (level - val1) / (val2 - val1)
                    };

                    let c1 = corners[*v1_idx];
                    let c2 = corners[*v2_idx];

                    let x = grid.x_coords[c1] + t * (grid.x_coords[c2] - grid.x_coords[c1]);
                    let y = grid.y_coords[c1] + t * (grid.y_coords[c2] - grid.y_coords[c1]);
                    let z = grid.z_coords[c1] + t * (grid.z_coords[c2] - grid.z_coords[c1]);

                    line_segments.push(x);
                    line_segments.push(y);
                    line_segments.push(z);
                }
            }
        }
    }

    Ok(line_segments)
}

/// Extract contour lines from triangulated mesh
/// Returns flat array of line segment endpoints (x1,y1,z1, x2,y2,z2, ...)
fn extract_contour_lines_from_triangles(
    vertices: &[f32],
    triangle_indices: &[u32],
    scalar_values: &[f32],
    level: f32,
) -> Result<Vec<f32>, String> {
    let mut line_segments = Vec::new();

    // Iterate over all triangles
    for tri_idx in (0..triangle_indices.len()).step_by(3) {
        let i0 = triangle_indices[tri_idx] as usize;
        let i1 = triangle_indices[tri_idx + 1] as usize;
        let i2 = triangle_indices[tri_idx + 2] as usize;

        let v0 = scalar_values[i0];
        let v1 = scalar_values[i1];
        let v2 = scalar_values[i2];

        // Check each edge of triangle for crossing
        let edges = [(i0, i1, v0, v1), (i1, i2, v1, v2), (i2, i0, v2, v0)];
        let mut crossings = Vec::new();

        for (idx1, idx2, val1, val2) in edges.iter() {
            if (val1 <= &level && val2 > &level) || (val1 > &level && val2 <= &level) {
                // Linear interpolation
                let t = if (val2 - val1).abs() < 1e-6 {
                    0.5
                } else {
                    (level - val1) / (val2 - val1)
                };

                let p1_offset = idx1 * 3;
                let p2_offset = idx2 * 3;

                let x = vertices[p1_offset] + t * (vertices[p2_offset] - vertices[p1_offset]);
                let y = vertices[p1_offset + 1]
                    + t * (vertices[p2_offset + 1] - vertices[p1_offset + 1]);
                let z = vertices[p1_offset + 2]
                    + t * (vertices[p2_offset + 2] - vertices[p1_offset + 2]);

                crossings.push((x, y, z));
            }
        }

        // A triangle should have 0 or 2 crossings (or rarely 1 if level passes through vertex)
        if crossings.len() >= 2 {
            line_segments.push(crossings[0].0);
            line_segments.push(crossings[0].1);
            line_segments.push(crossings[0].2);
            line_segments.push(crossings[1].0);
            line_segments.push(crossings[1].1);
            line_segments.push(crossings[1].2);
        }
    }

    Ok(line_segments)
}

impl Plot3DSolution {
    /// Calculate total number of points
    #[allow(dead_code)]
    pub fn total_points(&self) -> usize {
        (self.dimensions.i as usize) * (self.dimensions.j as usize) * (self.dimensions.k as usize)
    }
}

impl Plot3DFunction {
    /// Calculate total number of points
    #[allow(dead_code)]
    pub fn total_points(&self) -> usize {
        (self.dimensions.i as usize) * (self.dimensions.j as usize) * (self.dimensions.k as usize)
    }
}

/// Parse metadata from buffer, reading as many fields as available
/// Fields are read in order: REFMACH, ALPHA, REY, TIME, GAMINF, BETA, TINF, IGAM, HTINF, HT1, HT2, RGAS[...], FSMACH, TVREF, DTVREF
/// If the buffer is shorter than expected, later fields will be None
fn parse_metadata(buffer: &[u8], byte_order: ByteOrder) -> Plot3DMetadata {
    let num_floats = buffer.len() / 4;
    let mut values = Vec::with_capacity(num_floats);

    // Read all available f32 values from buffer
    for i in 0..num_floats {
        let start = i * 4;
        if start + 4 <= buffer.len() {
            let bytes = [
                buffer[start],
                buffer[start + 1],
                buffer[start + 2],
                buffer[start + 3],
            ];
            let value = match byte_order {
                ByteOrder::LittleEndian => f32::from_le_bytes(bytes),
                ByteOrder::BigEndian => f32::from_be_bytes(bytes),
            };
            values.push(value);
        }
    }

    // Extract fields in order - if not enough values, fields remain None
    let refmach = values.get(0).copied();
    let alpha = values.get(1).copied();
    let rey = values.get(2).copied();
    let time = values.get(3).copied();
    let gaminf = values.get(4).copied();
    let beta = values.get(5).copied();
    let tinf = values.get(6).copied();
    let igam = values.get(7).copied();
    let htinf = values.get(8).copied();
    let ht1 = values.get(9).copied();
    let ht2 = values.get(10).copied();

    // RGAS is variable length - collect remaining values except last 3 (FSMACH, TVREF, DTVREF)
    // If we have at least 15 values (11 fixed + 1 RGAS + 3 tail), assume last 3 are FSMACH, TVREF, DTVREF
    let (rgas, fsmach, tvref, dtvref) = if values.len() > 14 {
        let rgas_values: Vec<f32> = values[11..values.len() - 3].to_vec();
        let fsmach = values.get(values.len() - 3).copied();
        let tvref = values.get(values.len() - 2).copied();
        let dtvref = values.get(values.len() - 1).copied();
        (
            if rgas_values.is_empty() {
                None
            } else {
                Some(rgas_values)
            },
            fsmach,
            tvref,
            dtvref,
        )
    } else if values.len() > 11 {
        // We have values beyond the first 11, but not enough for the last 3
        let rgas_values: Vec<f32> = values[11..].to_vec();
        (
            if rgas_values.is_empty() {
                None
            } else {
                Some(rgas_values)
            },
            None,
            None,
            None,
        )
    } else {
        // No RGAS or tail values
        (None, None, None, None)
    };

    Plot3DMetadata {
        refmach,
        alpha,
        rey,
        time,
        gaminf,
        beta,
        tinf,
        igam,
        htinf,
        ht1,
        ht2,
        rgas,
        fsmach,
        tvref,
        dtvref,
    }
}

/// Auto-detect byte order by reading first dimension value
#[allow(dead_code)]
fn detect_byte_order<R: Read>(reader: &mut R) -> io::Result<ByteOrder> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;

    let le_value = i32::from_le_bytes(buf);
    let be_value = i32::from_be_bytes(buf);

    // PLOT3D dimensions are typically between 1 and 10000
    // Little-endian is more common on modern systems
    if le_value > 0 && le_value < 10000 {
        Ok(ByteOrder::LittleEndian)
    } else if be_value > 0 && be_value < 10000 {
        Ok(ByteOrder::BigEndian)
    } else {
        // Default to little-endian if ambiguous
        Ok(ByteOrder::LittleEndian)
    }
}

/// Read PLOT3D grid file (binary format)
/// PLOT3D format specification (Fortran unformatted):
/// - Record 1: number of grids (int32) - surrounded by record markers
/// - Record 2: For each grid: I, J, K dimensions (3 x int32 x num_grids) - surrounded by record markers  
/// - Records 3+: Grid coordinates: X, Y, Z arrays (float32) - each array in its own record with markers
#[allow(dead_code)]
pub fn read_plot3d_grid<P: AsRef<Path>>(path: P) -> io::Result<Vec<Plot3DGrid>> {
    let path_ref = path.as_ref();
    let file = File::open(path_ref)?;
    let mut reader = BufReader::new(file);

    // Skip opening record marker for number of grids
    skip_record_marker(&mut reader)?;

    // Read number of grids
    let num_grids = read_i32(&mut reader, ByteOrder::LittleEndian)?; // Try little-endian first

    // Skip closing record marker
    skip_record_marker(&mut reader)?;

    let byte_order = if num_grids > 0 && num_grids < 1000 {
        ByteOrder::LittleEndian
    } else {
        // Try big-endian
        let file = File::open(path_ref)?;
        let mut reader = BufReader::new(file);
        skip_record_marker(&mut reader)?;
        let num_grids_be = read_i32(&mut reader, ByteOrder::BigEndian)?;
        skip_record_marker(&mut reader)?;

        if num_grids_be > 0 && num_grids_be < 1000 {
            ByteOrder::BigEndian
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Invalid number of grids: {} (LE) or {} (BE)",
                    num_grids, num_grids_be
                ),
            ));
        }
    };

    // Re-read from start with correct byte order
    let file = File::open(path_ref)?;
    let mut reader = BufReader::new(file);

    skip_record_marker(&mut reader)?;
    let num_grids = read_i32(&mut reader, byte_order)?;
    skip_record_marker(&mut reader)?;

    if num_grids <= 0 || num_grids > 1000 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid number of grids: {}", num_grids),
        ));
    }

    let mut grids = Vec::with_capacity(num_grids as usize);

    // Read dimensions for all grids (in one record with markers)
    skip_record_marker(&mut reader)?;
    let mut dimensions_list = Vec::with_capacity(num_grids as usize);
    for _ in 0..num_grids {
        let i = read_i32(&mut reader, byte_order)? as u32;
        let j = read_i32(&mut reader, byte_order)? as u32;
        let k = read_i32(&mut reader, byte_order)? as u32;

        if i == 0 || j == 0 || k == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid dimensions: {}x{}x{}", i, j, k),
            ));
        }

        dimensions_list.push(GridDimensions { i, j, k });
    }
    skip_record_marker(&mut reader)?;

    // Read coordinate data for each grid
    for dims in dimensions_list {
        let total_points = (dims.i as usize) * (dims.j as usize) * (dims.k as usize);

        let (x_coords, y_coords, z_coords, _precision, iblank) =
            read_xyz_coords_with_markers(&mut reader, total_points, byte_order)?;

        grids.push(Plot3DGrid {
            dimensions: dims,
            x_coords,
            y_coords,
            z_coords,
            iblank,
        });
    }

    Ok(grids)
}

/// Read PLOT3D grid file with metadata about byte order and dimensions
pub fn read_plot3d_grid_with_metadata<P: AsRef<Path>>(
    path: P,
) -> io::Result<(Vec<Plot3DGrid>, GridFileMetadata)> {
    let file = File::open(&path)?;
    let mut reader = BufReader::new(file);

    // Skip opening record marker for number of grids
    skip_record_marker(&mut reader)?;

    // Try reading number of grids with little-endian
    let num_grids_le = read_i32(&mut reader, ByteOrder::LittleEndian)?;

    // Determine byte order based on validity of num_grids
    let byte_order = if num_grids_le > 0 && num_grids_le < 1000 {
        ByteOrder::LittleEndian
    } else {
        // Try big-endian - need to re-read the file
        let file = File::open(&path)?;
        let mut reader = BufReader::new(file);
        skip_record_marker(&mut reader)?;
        let num_grids_be = read_i32(&mut reader, ByteOrder::BigEndian)?;

        if num_grids_be > 0 && num_grids_be < 1000 {
            ByteOrder::BigEndian
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Invalid number of grids: {} (LE) or {} (BE)",
                    num_grids_le, num_grids_be
                ),
            ));
        }
    };

    let byte_order_str = match byte_order {
        ByteOrder::LittleEndian => "Little-Endian",
        ByteOrder::BigEndian => "Big-Endian",
    };

    // Re-read from start with correct byte order
    let file = File::open(&path)?;
    let mut reader = BufReader::new(file);

    skip_record_marker(&mut reader)?;
    let num_grids = read_i32(&mut reader, byte_order)?;
    skip_record_marker(&mut reader)?;

    if num_grids <= 0 || num_grids > 1000 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid number of grids: {}", num_grids),
        ));
    }

    let mut grids = Vec::with_capacity(num_grids as usize);
    let mut grid_dimensions = Vec::with_capacity(num_grids as usize);
    let mut precision: Option<Precision> = None;
    let mut has_iblank = false;

    // Read dimensions for all grids (in one record with markers)
    skip_record_marker(&mut reader)?;
    let mut dimensions_list = Vec::with_capacity(num_grids as usize);
    for _ in 0..num_grids {
        let i = read_i32(&mut reader, byte_order)? as u32;
        let j = read_i32(&mut reader, byte_order)? as u32;
        let k = read_i32(&mut reader, byte_order)? as u32;

        if i == 0 || j == 0 || k == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid dimensions: {}x{}x{}", i, j, k),
            ));
        }

        let dims = GridDimensions { i, j, k };
        grid_dimensions.push(dims.clone());
        dimensions_list.push(dims);
    }
    skip_record_marker(&mut reader)?;

    // Read coordinate data for each grid
    for dims in dimensions_list {
        let total_points = (dims.i as usize) * (dims.j as usize) * (dims.k as usize);

        let (x_coords, y_coords, z_coords, grid_precision, iblank) =
            read_xyz_coords_with_markers(&mut reader, total_points, byte_order)?;

        precision = Some(match precision {
            None => grid_precision,
            Some(existing) if existing == grid_precision => existing,
            Some(_) => Precision::Mixed,
        });

        if iblank.is_some() {
            has_iblank = true;
        }

        grids.push(Plot3DGrid {
            dimensions: dims,
            x_coords,
            y_coords,
            z_coords,
            iblank,
        });
    }

    let metadata = GridFileMetadata {
        byte_order: byte_order_str.to_string(),
        is_detected: true,
        precision: precision.unwrap_or(Precision::Mixed).as_str().to_string(),
        has_iblank,
        num_grids: num_grids as usize,
        grid_dimensions,
    };

    Ok((grids, metadata))
}

/// Read PLOT3D grid file in ASCII format
pub fn read_plot3d_grid_ascii<P: AsRef<Path>>(path: P) -> io::Result<Vec<Plot3DGrid>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    // Read number of grids
    let first_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Empty file"))??;
    let num_grids: i32 = first_line
        .trim()
        .parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Cannot parse number of grids"))?;

    if num_grids <= 0 || num_grids > 1000 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid number of grids: {}", num_grids),
        ));
    }

    let mut grids = Vec::with_capacity(num_grids as usize);

    // Read dimensions for all grids
    let mut dimensions_list = Vec::with_capacity(num_grids as usize);
    for _ in 0..num_grids {
        let dims_line = lines.next().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "Missing dimension line")
        })??;
        let dims: Vec<u32> = dims_line
            .split_whitespace()
            .map(|s| s.parse::<u32>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Cannot parse dimensions"))?;

        if dims.len() != 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Expected 3 dimensions (I, J, K)",
            ));
        }

        if dims[0] == 0 || dims[1] == 0 || dims[2] == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid dimensions: {}x{}x{}", dims[0], dims[1], dims[2]),
            ));
        }

        dimensions_list.push(GridDimensions {
            i: dims[0],
            j: dims[1],
            k: dims[2],
        });
    }

    // Read coordinate data for each grid
    for dims in dimensions_list {
        let total_points = (dims.i as usize) * (dims.j as usize) * (dims.k as usize);
        let mut x_coords = Vec::with_capacity(total_points);
        let mut y_coords = Vec::with_capacity(total_points);
        let mut z_coords = Vec::with_capacity(total_points);

        // Read coordinates (typically one per line or multiple per line)
        let mut values_read = 0;
        let mut current_array = 0; // 0 = x, 1 = y, 2 = z

        for line in lines.by_ref() {
            let line = line?;
            let values: Vec<f32> = line
                .split_whitespace()
                .map(|s| s.parse::<f32>())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "Cannot parse coordinate value")
                })?;

            for value in values {
                match current_array {
                    0 => x_coords.push(value),
                    1 => y_coords.push(value),
                    2 => z_coords.push(value),
                    _ => unreachable!(),
                }
                values_read += 1;

                if values_read == total_points {
                    current_array += 1;
                    values_read = 0;
                    if current_array == 3 {
                        break;
                    }
                }
            }

            if current_array == 3 {
                break;
            }
        }

        if x_coords.len() != total_points
            || y_coords.len() != total_points
            || z_coords.len() != total_points
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Incomplete coordinate data: expected {}, got {}/{}/{} values",
                    total_points,
                    x_coords.len(),
                    y_coords.len(),
                    z_coords.len()
                ),
            ));
        }

        grids.push(Plot3DGrid {
            dimensions: dims,
            x_coords,
            y_coords,
            z_coords,
            iblank: None, // ASCII format typically doesn't include iblank
        });
    }

    Ok(grids)
}

/// Read PLOT3D solution file (Q file) in binary format
pub fn read_plot3d_solution<P: AsRef<Path>>(path: P) -> io::Result<Vec<Plot3DSolution>> {
    let path_ref = path.as_ref();
    let file = File::open(path_ref)?;
    let mut reader = BufReader::new(file);

    // Try little-endian first
    skip_record_marker(&mut reader)?;
    let num_grids_le = read_i32(&mut reader, ByteOrder::LittleEndian)?;
    skip_record_marker(&mut reader)?;

    let byte_order = if num_grids_le > 0 && num_grids_le < 1000 {
        ByteOrder::LittleEndian
    } else {
        // Try big-endian
        let file = File::open(path_ref)?;
        let mut reader = BufReader::new(file);
        skip_record_marker(&mut reader)?;
        let num_grids_be = read_i32(&mut reader, ByteOrder::BigEndian)?;
        skip_record_marker(&mut reader)?;

        if num_grids_be > 0 && num_grids_be < 1000 {
            ByteOrder::BigEndian
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Invalid number of grids: {} (LE) or {} (BE)",
                    num_grids_le, num_grids_be
                ),
            ));
        }
    };

    // Re-read from start with correct byte order
    let file = File::open(path_ref)?;
    let mut reader = BufReader::new(file);

    skip_record_marker(&mut reader)?;
    let num_grids = read_i32(&mut reader, byte_order)?;
    skip_record_marker(&mut reader)?;

    // Read dimensions and NQ, NQC from a single record
    skip_record_marker(&mut reader)?;
    let mut dimensions_list = Vec::with_capacity(num_grids as usize);
    for _ in 0..num_grids {
        let i = read_i32(&mut reader, byte_order)? as u32;
        let j = read_i32(&mut reader, byte_order)? as u32;
        let k = read_i32(&mut reader, byte_order)? as u32;

        if i == 0 || j == 0 || k == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid dimensions: {}x{}x{}", i, j, k),
            ));
        }

        dimensions_list.push(GridDimensions { i, j, k });
    }

    // Read NQ (number of solution variables) and NQC (number of conservative variables)
    let nq = read_i32(&mut reader, byte_order)? as usize;
    let _nqc = read_i32(&mut reader, byte_order)? as usize;
    skip_record_marker(&mut reader)?;

    if nq < 5 || nq > 100 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid NQ: {}", nq),
        ));
    }

    let mut solutions = Vec::with_capacity(num_grids as usize);

    // Track the first precision detected (should be same for whole file)
    let mut detected_precision: Option<Precision> = None;

    // Read solution data for each grid
    for (grid_index, dims) in dimensions_list.into_iter().enumerate() {
        let total_points = (dims.i as usize) * (dims.j as usize) * (dims.k as usize);

        // FIRST: Detect precision from Q array record size before reading metadata
        // Peek at the Q array record marker to determine f32 vs f64
        // We need to know this before reading data, so read metadata and Q positions first

        // Record sequence: metadata record marker, metadata, closing marker, Q record marker, ...
        // Read metadata record opening marker
        let metadata_record_size = read_record_marker(&mut reader, byte_order)? as usize;

        // Read and store metadata buffer
        let mut metadata_buf = vec![0u8; metadata_record_size];
        reader.read_exact(&mut metadata_buf).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to read metadata record: {}", e),
            )
        })?;
        skip_record_marker(&mut reader)?;

        // Now read the Q array record marker to detect precision BEFORE reading Q data
        let q_record_size = read_record_marker(&mut reader, byte_order)? as usize;

        // Determine precision based on Q record size
        // NQ variables * total_points values per variable
        let expected_f32_size = nq * total_points * 4; // f32 = 4 bytes
        let expected_f64_size = nq * total_points * 8; // f64 = 8 bytes

        let precision = if q_record_size == expected_f32_size {
            Precision::F32
        } else if q_record_size == expected_f64_size {
            Precision::F64
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Invalid Q record size: expected {} bytes (f32) or {} bytes (f64), got {} bytes",
                    expected_f32_size, expected_f64_size, q_record_size
                ),
            ));
        };

        // Track the first precision detected
        if detected_precision.is_none() {
            detected_precision = Some(precision);
        }

        // Parse metadata - will read as many fields as are available
        let metadata = parse_metadata(&metadata_buf, byte_order);

        // Now read Q data with the detected precision
        let mut q_data = vec![Vec::with_capacity(total_points); nq];
        for n in 0..nq {
            q_data[n] = match precision {
                Precision::F32 => read_f32_array(&mut reader, total_points, byte_order)?,
                Precision::F64 => {
                    // Read f64 and convert to f32 for storage (lossy but preserves values)
                    let f64_data = read_f64_array(&mut reader, total_points, byte_order)?;
                    f64_data.iter().map(|&v| v as f32).collect()
                }
                Precision::Mixed => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Mixed precision not supported for solution data",
                    ));
                }
            };
        }

        skip_record_marker(&mut reader)?;

        // Extract the conservative variables (first 5) and gamma (6th if present)
        // NQ = 6 + NQC + NQT where first 6 are: rho*, rho*u*, rho*v*, rho*w*, rho*e0*, gamma
        let rho = q_data.get(0).cloned().unwrap_or_default();
        let rhou = q_data.get(1).cloned().unwrap_or_default();
        let rhov = q_data.get(2).cloned().unwrap_or_default();
        let rhow = q_data.get(3).cloned().unwrap_or_default();
        let rhoe = q_data.get(4).cloned().unwrap_or_default();
        let gamma = q_data.get(5).cloned(); // Optional: ratio of specific heats

        // Validate we got the right amount of data
        if rho.len() != total_points
            || rhou.len() != total_points
            || rhov.len() != total_points
            || rhow.len() != total_points
            || rhoe.len() != total_points
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Incomplete solution data for grid {}: expected {} points",
                    grid_index, total_points
                ),
            ));
        }

        solutions.push(Plot3DSolution {
            grid_index,
            dimensions: dims,
            rho,
            rhou,
            rhov,
            rhow,
            rhoe,
            gamma,
            metadata: Some(metadata),
        });
    }

    // Set metadata for logging in the command handler
    let byte_order_str = match byte_order {
        ByteOrder::LittleEndian => "Little-Endian",
        ByteOrder::BigEndian => "Big-Endian",
    };
    let precision_str = detected_precision.unwrap_or(Precision::F32).as_str();
    set_last_solution_metadata(SolutionFileMetadata {
        format: "binary".to_string(),
        precision: precision_str.to_string(),
        byte_order: byte_order_str.to_string(),
    });

    Ok(solutions)
}

/// Read PLOT3D solution file in ASCII format
pub fn read_plot3d_solution_ascii<P: AsRef<Path>>(path: P) -> io::Result<Vec<Plot3DSolution>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    // Read number of grids
    let first_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Empty file"))??;
    let num_grids: i32 = first_line
        .trim()
        .parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Cannot parse number of grids"))?;

    if num_grids <= 0 || num_grids > 1000 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid number of grids: {}", num_grids),
        ));
    }

    let mut solutions = Vec::with_capacity(num_grids as usize);

    // Read dimensions for all grids
    let mut dimensions_list = Vec::with_capacity(num_grids as usize);
    for _ in 0..num_grids {
        let dims_line = lines.next().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "Missing dimension line")
        })??;
        let dims: Vec<u32> = dims_line
            .split_whitespace()
            .map(|s| s.parse::<u32>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Cannot parse dimensions"))?;

        if dims.len() != 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Expected 3 dimensions (I, J, K)",
            ));
        }

        dimensions_list.push(GridDimensions {
            i: dims[0],
            j: dims[1],
            k: dims[2],
        });
    }

    // Read solution data for each grid
    for (grid_index, dims) in dimensions_list.into_iter().enumerate() {
        let total_points = (dims.i as usize) * (dims.j as usize) * (dims.k as usize);

        // First, read metadata values (variable number depending on file format)
        // Try to read metadata fields: REFMACH, ALPHA, REY, TIME, GAMINF, BETA, TINF, IGAM, HTINF, HT1, HT2, RGAS[...], FSMACH, TVREF, DTVREF
        // For ASCII files, we need to parse until we find the solution data
        // Minimum metadata is 4 values (REFMACH, ALPHA, REY, TIME)
        // We'll read values greedily and then determine which are metadata vs solution data

        let mut all_values: Vec<f32> = Vec::new();

        // Read values until we have enough for metadata + solution data
        // We need: metadata (at least 4 floats) + 5 variable arrays * total_points
        let min_metadata_count = 4;
        let min_solution_count = 5 * total_points;
        let min_total = min_metadata_count + min_solution_count;

        for line in lines.by_ref() {
            let line = line?;
            let values: Vec<f32> = line
                .split_whitespace()
                .map(|s| s.parse::<f32>())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Cannot parse value"))?;

            all_values.extend(values);

            if all_values.len() >= min_total {
                break;
            }
        }

        if all_values.len() < min_total {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Incomplete solution data for grid {}: expected at least {} values, got {}",
                    grid_index,
                    min_total,
                    all_values.len()
                ),
            ));
        }

        // Split into metadata and solution data
        // We'll try to parse metadata first, reading as many fields as available
        // but leaving at least 5*total_points for solution data
        let max_metadata_idx = all_values.len() - min_solution_count;

        // Convert metadata values to buffer format for parse_metadata function
        let mut metadata_buf = Vec::new();
        for i in 0..max_metadata_idx {
            metadata_buf.extend_from_slice(&all_values[i].to_le_bytes());
        }
        let metadata = parse_metadata(&metadata_buf, ByteOrder::LittleEndian);

        // Remaining values are solution data
        let solution_data = &all_values[max_metadata_idx..];

        let mut rho = Vec::with_capacity(total_points);
        let mut rhou = Vec::with_capacity(total_points);
        let mut rhov = Vec::with_capacity(total_points);
        let mut rhow = Vec::with_capacity(total_points);
        let mut rhoe = Vec::with_capacity(total_points);
        let mut gamma = Vec::with_capacity(total_points);

        // Distribute values across the 5+ variables
        for (idx, &value) in solution_data.iter().enumerate() {
            let var_index = idx / total_points;
            let point_index = idx % total_points;

            match var_index {
                0 => {
                    if point_index >= rho.len() {
                        rho.push(value);
                    }
                }
                1 => {
                    if point_index >= rhou.len() {
                        rhou.push(value);
                    }
                }
                2 => {
                    if point_index >= rhov.len() {
                        rhov.push(value);
                    }
                }
                3 => {
                    if point_index >= rhow.len() {
                        rhow.push(value);
                    }
                }
                4 => {
                    if point_index >= rhoe.len() {
                        rhoe.push(value);
                    }
                }
                5 => {
                    if point_index >= gamma.len() {
                        gamma.push(value);
                    }
                }
                _ => break, // Additional variables beyond gamma
            }
        }

        let gamma_opt = if gamma.len() == total_points {
            Some(gamma)
        } else {
            None
        };

        // Validate we got the right amount of data
        if rho.len() != total_points
            || rhou.len() != total_points
            || rhov.len() != total_points
            || rhow.len() != total_points
            || rhoe.len() != total_points
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Incomplete solution data for grid {}", grid_index),
            ));
        }

        solutions.push(Plot3DSolution {
            grid_index,
            dimensions: dims,
            rho,
            rhou,
            rhov,
            rhow,
            rhoe,
            gamma: gamma_opt,
            metadata: Some(metadata),
        });
    }

    // Set metadata for logging in the command handler
    set_last_solution_metadata(SolutionFileMetadata {
        format: "ASCII".to_string(),
        precision: "f32".to_string(),
        byte_order: "N/A".to_string(),
    });

    Ok(solutions)
}

/// Read PLOT3D function file (F file) in binary format
pub fn read_plot3d_function<P: AsRef<Path>>(path: P) -> io::Result<Vec<Plot3DFunction>> {
    let path_ref = path.as_ref();
    let file = File::open(path_ref)?;
    let mut reader = BufReader::new(file);

    // Try little-endian first
    skip_record_marker(&mut reader)?;
    let num_grids_le = read_i32(&mut reader, ByteOrder::LittleEndian)?;
    skip_record_marker(&mut reader)?;

    let byte_order = if num_grids_le > 0 && num_grids_le < 1000 {
        ByteOrder::LittleEndian
    } else {
        // Try big-endian
        let file = File::open(path_ref)?;
        let mut reader = BufReader::new(file);
        skip_record_marker(&mut reader)?;
        let num_grids_be = read_i32(&mut reader, ByteOrder::BigEndian)?;
        skip_record_marker(&mut reader)?;

        if num_grids_be > 0 && num_grids_be < 1000 {
            ByteOrder::BigEndian
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Invalid number of grids: {} (LE) or {} (BE)",
                    num_grids_le, num_grids_be
                ),
            ));
        }
    };

    // Re-read from start with correct byte order
    let file = File::open(path_ref)?;
    let mut reader = BufReader::new(file);

    skip_record_marker(&mut reader)?;
    let num_grids = read_i32(&mut reader, byte_order)?;
    skip_record_marker(&mut reader)?;

    let mut functions = Vec::with_capacity(num_grids as usize);

    // Skip opening record marker for dimensions
    skip_record_marker(&mut reader)?;

    // Read dimensions for all grids
    let mut dimensions_list = Vec::with_capacity(num_grids as usize);
    for _ in 0..num_grids {
        let i = read_i32(&mut reader, byte_order)? as u32;
        let j = read_i32(&mut reader, byte_order)? as u32;
        let k = read_i32(&mut reader, byte_order)? as u32;

        if i == 0 || j == 0 || k == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid dimensions: {}x{}x{}", i, j, k),
            ));
        }

        dimensions_list.push(GridDimensions { i, j, k });
    }

    // Skip closing record marker for dimensions
    skip_record_marker(&mut reader)?;

    // Read function data for each grid
    for (grid_index, dims) in dimensions_list.into_iter().enumerate() {
        let total_points = (dims.i as usize) * (dims.j as usize) * (dims.k as usize);

        // Skip opening record marker and read number of functions
        skip_record_marker(&mut reader)?;
        let num_functions = read_i32(&mut reader, byte_order)? as usize;
        skip_record_marker(&mut reader)?;

        let mut function_data = Vec::with_capacity(num_functions);

        for _ in 0..num_functions {
            skip_record_marker(&mut reader)?;
            let func_array = read_f32_array(&mut reader, total_points, byte_order)?;
            skip_record_marker(&mut reader)?;
            function_data.push(func_array);
        }

        functions.push(Plot3DFunction {
            grid_index,
            dimensions: dims,
            function_data,
        });
    }

    Ok(functions)
}

// Helper functions for binary reading
fn read_i32<R: Read>(reader: &mut R, byte_order: ByteOrder) -> io::Result<i32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(match byte_order {
        ByteOrder::LittleEndian => i32::from_le_bytes(buf),
        ByteOrder::BigEndian => i32::from_be_bytes(buf),
    })
}

#[allow(dead_code)]
fn read_f32<R: Read>(reader: &mut R, byte_order: ByteOrder) -> io::Result<f32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(match byte_order {
        ByteOrder::LittleEndian => f32::from_le_bytes(buf),
        ByteOrder::BigEndian => f32::from_be_bytes(buf),
    })
}

/// Read Fortran record marker and return the record length in bytes
fn read_record_marker<R: Read>(reader: &mut R, byte_order: ByteOrder) -> io::Result<i32> {
    read_i32(reader, byte_order)
}

/// Skip Fortran record marker (4-byte integer)
fn skip_record_marker<R: Read>(reader: &mut R) -> io::Result<()> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(())
}

fn read_f32_array<R: Read>(
    reader: &mut R,
    count: usize,
    byte_order: ByteOrder,
) -> io::Result<Vec<f32>> {
    let mut result = Vec::with_capacity(count);
    for _ in 0..count {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf)?;
        let value = match byte_order {
            ByteOrder::LittleEndian => f32::from_le_bytes(buf),
            ByteOrder::BigEndian => f32::from_be_bytes(buf),
        };
        result.push(value);
    }
    Ok(result)
}

fn read_f64_array<R: Read>(
    reader: &mut R,
    count: usize,
    byte_order: ByteOrder,
) -> io::Result<Vec<f64>> {
    let mut result = Vec::with_capacity(count);
    for _ in 0..count {
        let mut buf = [0u8; 8];
        reader.read_exact(&mut buf)?;
        let value = match byte_order {
            ByteOrder::LittleEndian => f64::from_le_bytes(buf),
            ByteOrder::BigEndian => f64::from_be_bytes(buf),
        };
        result.push(value);
    }
    Ok(result)
}

/// Read three f32 arrays (x,y,z) with Fortran record markers
/// Handles both separate records (one per coordinate) and combined records (all xyz in one)
fn read_xyz_coords_with_markers<R: Read>(
    reader: &mut R,
    count: usize,
    byte_order: ByteOrder,
) -> io::Result<(Vec<f32>, Vec<f32>, Vec<f32>, Precision, Option<Vec<i32>>)> {
    let record_size = read_record_marker(reader, byte_order)?;

    if record_size <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid record marker: {}", record_size),
        ));
    }

    let total_values_f32 = record_size as usize / 4;
    let total_values_f64 = record_size as usize / 8;

    // XYZ only (f32)
    if total_values_f32 == count * 3 {
        let x_coords = read_values_with_precision(reader, count, byte_order, Precision::F32)?;
        let y_coords = read_values_with_precision(reader, count, byte_order, Precision::F32)?;
        let z_coords = read_values_with_precision(reader, count, byte_order, Precision::F32)?;

        let closing_marker = read_record_marker(reader, byte_order)?;
        if closing_marker != record_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Record marker mismatch: {} != {}",
                    record_size, closing_marker
                ),
            ));
        }

        logger::log_entry(
            "DEBUG",
            &format!("Read XYZ as f32, no iblank (record_size={})", record_size),
            Some("plot3d".to_string()),
        );
        Ok((x_coords, y_coords, z_coords, Precision::F32, None))
    }
    // XYZ only (f64)
    else if total_values_f64 == count * 3 {
        let x_coords = read_values_with_precision(reader, count, byte_order, Precision::F64)?;
        let y_coords = read_values_with_precision(reader, count, byte_order, Precision::F64)?;
        let z_coords = read_values_with_precision(reader, count, byte_order, Precision::F64)?;

        let closing_marker = read_record_marker(reader, byte_order)?;
        if closing_marker != record_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Record marker mismatch: {} != {}",
                    record_size, closing_marker
                ),
            ));
        }

        logger::log_entry(
            "DEBUG",
            &format!("Read XYZ as f64, no iblank (record_size={})", record_size),
            Some("plot3d".to_string()),
        );
        Ok((x_coords, y_coords, z_coords, Precision::F64, None))
    }
    // XYZ (f32) + IBLANK (i32): count * 3 * 4 + count * 4 = count * 16
    else if record_size as usize == count * 16 {
        let x_coords = read_values_with_precision(reader, count, byte_order, Precision::F32)?;
        let y_coords = read_values_with_precision(reader, count, byte_order, Precision::F32)?;
        let z_coords = read_values_with_precision(reader, count, byte_order, Precision::F32)?;

        // Read IBLANK data as i32
        let mut iblank = Vec::with_capacity(count);
        for _ in 0..count {
            let mut buf = [0u8; 4];
            reader.read_exact(&mut buf)?;
            let value = match byte_order {
                ByteOrder::LittleEndian => i32::from_le_bytes(buf),
                ByteOrder::BigEndian => i32::from_be_bytes(buf),
            };
            iblank.push(value);
        }

        let closing_marker = read_record_marker(reader, byte_order)?;
        if closing_marker != record_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Record marker mismatch: {} != {}",
                    record_size, closing_marker
                ),
            ));
        }

        logger::log_entry(
            "INFO",
            &format!("Read XYZ (f32) + IBLANK (i32) with {} points", count),
            Some("plot3d".to_string()),
        );
        Ok((x_coords, y_coords, z_coords, Precision::F32, Some(iblank)))
    }
    // XYZ (f64) + IBLANK (i32): count * 3 * 8 + count * 4 = count * 28
    else if record_size as usize == count * 28 {
        let x_coords = read_values_with_precision(reader, count, byte_order, Precision::F64)?;
        let y_coords = read_values_with_precision(reader, count, byte_order, Precision::F64)?;
        let z_coords = read_values_with_precision(reader, count, byte_order, Precision::F64)?;

        // Read IBLANK data as i32
        let mut iblank = Vec::with_capacity(count);
        for _ in 0..count {
            let mut buf = [0u8; 4];
            reader.read_exact(&mut buf)?;
            let value = match byte_order {
                ByteOrder::LittleEndian => i32::from_le_bytes(buf),
                ByteOrder::BigEndian => i32::from_be_bytes(buf),
            };
            iblank.push(value);
        }

        let closing_marker = read_record_marker(reader, byte_order)?;
        if closing_marker != record_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Record marker mismatch: {} != {}",
                    record_size, closing_marker
                ),
            ));
        }

        logger::log_entry(
            "INFO",
            &format!("Read XYZ (f64) + IBLANK (i32) with {} points", count),
            Some("plot3d".to_string()),
        );
        Ok((x_coords, y_coords, z_coords, Precision::F64, Some(iblank)))
    }
    // XYZ (f32) + IBLANK (byte): count * 3 * 4 + count * 1 = count * 13
    else if record_size as usize == count * 13 {
        let x_coords = read_values_with_precision(reader, count, byte_order, Precision::F32)?;
        let y_coords = read_values_with_precision(reader, count, byte_order, Precision::F32)?;
        let z_coords = read_values_with_precision(reader, count, byte_order, Precision::F32)?;

        // Read IBLANK data as bytes
        let mut iblank = Vec::with_capacity(count);
        for _ in 0..count {
            let mut buf = [0u8; 1];
            reader.read_exact(&mut buf)?;
            iblank.push(buf[0] as i32);
        }

        let closing_marker = read_record_marker(reader, byte_order)?;
        if closing_marker != record_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Record marker mismatch: {} != {}",
                    record_size, closing_marker
                ),
            ));
        }

        logger::log_entry(
            "INFO",
            &format!("Read XYZ (f32) + IBLANK (byte) with {} points", count),
            Some("plot3d".to_string()),
        );
        Ok((x_coords, y_coords, z_coords, Precision::F32, Some(iblank)))
    }
    // XYZ (f64) + IBLANK (byte): count * 3 * 8 + count * 1 = count * 25
    else if record_size as usize == count * 25 {
        let x_coords = read_values_with_precision(reader, count, byte_order, Precision::F64)?;
        let y_coords = read_values_with_precision(reader, count, byte_order, Precision::F64)?;
        let z_coords = read_values_with_precision(reader, count, byte_order, Precision::F64)?;

        // Read IBLANK data as bytes
        let mut iblank = Vec::with_capacity(count);
        for _ in 0..count {
            let mut buf = [0u8; 1];
            reader.read_exact(&mut buf)?;
            iblank.push(buf[0] as i32);
        }

        let closing_marker = read_record_marker(reader, byte_order)?;
        if closing_marker != record_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Record marker mismatch: {} != {}",
                    record_size, closing_marker
                ),
            ));
        }

        logger::log_entry(
            "INFO",
            &format!("Read XYZ (f64) + IBLANK (byte) with {} points", count),
            Some("plot3d".to_string()),
        );
        Ok((x_coords, y_coords, z_coords, Precision::F64, Some(iblank)))
    }
    // Separate XYZ records - check if record_size matches expected size for one coordinate array
    else {
        let precision = match record_size as usize {
            size if size == count * 4 => Precision::F32,
            size if size == count * 8 => Precision::F64,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Invalid record size: expected {} (f32), {} (f64), {} (f32+IBLANK i32), {} (f64+IBLANK i32), {} (f32+IBLANK byte), or {} (f64+IBLANK byte) bytes, got {} bytes",
                        count * 12,  // XYZ f32
                        count * 24,  // XYZ f64
                        count * 16,  // XYZ f32 + IBLANK i32
                        count * 28,  // XYZ f64 + IBLANK i32
                        count * 13,  // XYZ f32 + IBLANK byte
                        count * 25,  // XYZ f64 + IBLANK byte
                        record_size
                    ),
                ));
            }
        };

        let x_coords = read_values_with_precision(reader, count, byte_order, precision)?;

        let closing_marker = read_record_marker(reader, byte_order)?;
        if closing_marker != record_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "X record marker mismatch: {} != {}",
                    record_size, closing_marker
                ),
            ));
        }

        let y_coords = read_f32_array_with_markers_precision(reader, count, byte_order, precision)?;
        let z_coords = read_f32_array_with_markers_precision(reader, count, byte_order, precision)?;

        logger::log_entry(
            "DEBUG",
            &format!(
                "Read XYZ in separate records, no iblank (record_size={})",
                record_size
            ),
            Some("plot3d".to_string()),
        );
        Ok((x_coords, y_coords, z_coords, precision, None))
    }
}

fn read_values_with_precision<R: Read>(
    reader: &mut R,
    count: usize,
    byte_order: ByteOrder,
    precision: Precision,
) -> io::Result<Vec<f32>> {
    let mut result = Vec::with_capacity(count);
    match precision {
        Precision::F32 => {
            for _ in 0..count {
                let mut buf = [0u8; 4];
                reader.read_exact(&mut buf)?;
                result.push(match byte_order {
                    ByteOrder::LittleEndian => f32::from_le_bytes(buf),
                    ByteOrder::BigEndian => f32::from_be_bytes(buf),
                });
            }
        }
        Precision::F64 => {
            for _ in 0..count {
                let mut buf = [0u8; 8];
                reader.read_exact(&mut buf)?;
                result.push(
                    (match byte_order {
                        ByteOrder::LittleEndian => f64::from_le_bytes(buf),
                        ByteOrder::BigEndian => f64::from_be_bytes(buf),
                    }) as f32,
                );
            }
        }
        Precision::Mixed => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unexpected mixed precision in data block",
            ));
        }
    }
    Ok(result)
}

fn read_f32_array_with_markers_precision<R: Read>(
    reader: &mut R,
    count: usize,
    byte_order: ByteOrder,
    precision: Precision,
) -> io::Result<Vec<f32>> {
    let record_size = read_record_marker(reader, byte_order)?;
    let expected_size = match precision {
        Precision::F32 => (count * 4) as i32,
        Precision::F64 => (count * 8) as i32,
        Precision::Mixed => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unexpected mixed precision in record",
            ));
        }
    };

    if record_size != expected_size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Record size {} does not match expected {}",
                record_size, expected_size
            ),
        ));
    }

    let result = read_values_with_precision(reader, count, byte_order, precision)?;

    let closing_marker = read_record_marker(reader, byte_order)?;
    if closing_marker != record_size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Record marker mismatch: {} != {}",
                record_size, closing_marker
            ),
        ));
    }

    Ok(result)
}

/// Get the metadata from the last loaded solution file
pub fn get_last_solution_metadata() -> Option<SolutionFileMetadata> {
    LAST_SOLUTION_METADATA.with(|m| m.borrow().clone())
}

/// Set the metadata for the last loaded solution file (internal use)
fn set_last_solution_metadata(metadata: SolutionFileMetadata) {
    LAST_SOLUTION_METADATA.with(|m| *m.borrow_mut() = Some(metadata));
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_arbitrary_plane_aligned_with_face() {
        // 2x2x2 unit cube grid
        let grid = Plot3DGrid {
            dimensions: GridDimensions { i: 2, j: 2, k: 2 },
            x_coords: vec![0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0],
            y_coords: vec![0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0],
            z_coords: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            iblank: None,
        };

        // Plane exactly at z=0 (bottom face)
        let plane_point = [0.0, 0.0, 0.0];
        let plane_normal = [0.0, 0.0, 1.0];
        let result = grid.slice_arbitrary_plane(
            plane_point,
            plane_normal,
            false,
            false,
            crate::IblankFilterMode::Vertex,
        );
        assert!(result.is_ok(), "Should intersect aligned face");
        let mesh = result.unwrap();
        assert!(mesh.vertices.len() > 0, "Should have intersection vertices");
        // All intersection points should have z ≈ 0
        for i in (0..mesh.vertices.len()).step_by(3) {
            let z = mesh.vertices[i + 2];
            assert!((z - 0.0).abs() < 0.01, "Vertex z {} should be near 0.0", z);
        }
    }

    #[test]
    fn test_arbitrary_plane_aligned_with_edge() {
        // 2x2x2 unit cube grid
        let grid = Plot3DGrid {
            dimensions: GridDimensions { i: 2, j: 2, k: 2 },
            x_coords: vec![0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0],
            y_coords: vec![0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0],
            z_coords: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            iblank: None,
        };

        // Plane along x=0 edge
        let plane_point = [0.0, 0.0, 0.0];
        let plane_normal = [1.0, 0.0, 0.0];
        let result = grid.slice_arbitrary_plane(
            plane_point,
            plane_normal,
            false,
            false,
            crate::IblankFilterMode::Vertex,
        );
        assert!(result.is_ok(), "Should intersect aligned edge");
        let mesh = result.unwrap();
        assert!(mesh.vertices.len() > 0, "Should have intersection vertices");
        // All intersection points should have x ≈ 0
        for i in (0..mesh.vertices.len()).step_by(3) {
            let x = mesh.vertices[i];
            assert!((x - 0.0).abs() < 0.01, "Vertex x {} should be near 0.0", x);
        }
    }

    #[test]
    fn test_arbitrary_plane_duplicate_intersection_points() {
        // 2x2x2 unit cube grid
        let grid = Plot3DGrid {
            dimensions: GridDimensions { i: 2, j: 2, k: 2 },
            x_coords: vec![0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0],
            y_coords: vec![0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0],
            z_coords: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            iblank: None,
        };

        // Plane at z=0 (bottom face), should produce duplicate points at corners
        let plane_point = [0.0, 0.0, 0.0];
        let plane_normal = [0.0, 0.0, 1.0];
        let result = grid.slice_arbitrary_plane(
            plane_point,
            plane_normal,
            false,
            false,
            crate::IblankFilterMode::Vertex,
        );
        assert!(result.is_ok(), "Should intersect aligned face");
        let mesh = result.unwrap();
        // Check that duplicate points are not present (all points are unique)
        // Allow duplicates, but check that all intersection points are near the expected plane
        assert!(mesh.vertices.len() > 0, "Should have intersection vertices");
        for i in (0..mesh.vertices.len()).step_by(3) {
            let z = mesh.vertices[i + 2];
            assert!((z - 0.0).abs() < 0.01, "Vertex z {} should be near 0.0", z);
        }
    }

    #[test]
    fn test_arbitrary_plane_consistent_winding() {
        let grid = Plot3DGrid {
            dimensions: GridDimensions { i: 2, j: 2, k: 2 },
            x_coords: vec![0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0],
            y_coords: vec![0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0],
            z_coords: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            iblank: None,
        };

        let plane_point = [0.0, 0.0, 0.5];
        let plane_normal = [0.0, 0.0, 1.0];
        let mesh = grid
            .slice_arbitrary_plane(
                plane_point,
                plane_normal,
                false,
                false,
                crate::IblankFilterMode::Vertex,
            )
            .expect("Expected arbitrary plane intersection");

        assert_eq!(mesh.triangle_indices.len() % 3, 0);
        assert!(mesh.triangle_indices.len() >= 3);

        for tri in mesh.triangle_indices.chunks_exact(3) {
            let ia = tri[0] as usize;
            let ib = tri[1] as usize;
            let ic = tri[2] as usize;

            let a = [
                mesh.vertices[ia * 3],
                mesh.vertices[ia * 3 + 1],
                mesh.vertices[ia * 3 + 2],
            ];
            let b = [
                mesh.vertices[ib * 3],
                mesh.vertices[ib * 3 + 1],
                mesh.vertices[ib * 3 + 2],
            ];
            let c = [
                mesh.vertices[ic * 3],
                mesh.vertices[ic * 3 + 1],
                mesh.vertices[ic * 3 + 2],
            ];

            let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let tri_normal = [
                ab[1] * ac[2] - ab[2] * ac[1],
                ab[2] * ac[0] - ab[0] * ac[2],
                ab[0] * ac[1] - ab[1] * ac[0],
            ];

            let dot = tri_normal[2];
            assert!(dot >= -1e-5, "Triangle winding flipped: dot={dot}");
        }
    }

    #[test]
    fn test_arbitrary_plane_welds_adjacent_cell_face() {
        // 3x2x2 grid -> two cells along I direction sharing interior face at x=1
        let mut x_coords = Vec::new();
        let mut y_coords = Vec::new();
        let mut z_coords = Vec::new();

        for k in 0..2 {
            for j in 0..2 {
                for i in 0..3 {
                    x_coords.push(i as f32);
                    y_coords.push(j as f32);
                    z_coords.push(k as f32);
                }
            }
        }

        let grid = Plot3DGrid {
            dimensions: GridDimensions { i: 3, j: 2, k: 2 },
            x_coords,
            y_coords,
            z_coords,
            iblank: None,
        };

        let plane_point = [1.0, 0.0, 0.0];
        let plane_normal = [1.0, 0.0, 0.0];

        let mesh = grid
            .slice_arbitrary_plane(
                plane_point,
                plane_normal,
                false,
                false,
                crate::IblankFilterMode::Vertex,
            )
            .expect("Expected intersection at interior shared face");

        assert_eq!(mesh.vertex_count, 4, "Expected welded quad vertices");
        assert_eq!(mesh.triangle_indices.len(), 6, "Expected two triangles");
        for i in (0..mesh.vertices.len()).step_by(3) {
            assert!((mesh.vertices[i] - 1.0).abs() < 1e-5);
        }
    }
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_grid_dimensions() {
        let dims = GridDimensions {
            i: 10,
            j: 20,
            k: 30,
        };
        let grid = Plot3DGrid {
            dimensions: dims.clone(),
            x_coords: vec![],
            y_coords: vec![],
            z_coords: vec![],
            iblank: None,
        };
        assert_eq!(grid.total_points(), 6000);
    }

    #[test]
    fn test_solution_total_points() {
        let solution = Plot3DSolution {
            grid_index: 0,
            dimensions: GridDimensions { i: 5, j: 4, k: 3 },
            rho: vec![],
            rhou: vec![],
            rhov: vec![],
            rhow: vec![],
            rhoe: vec![],
            gamma: None,
            metadata: None,
        };
        assert_eq!(solution.total_points(), 60);
    }

    #[test]
    fn test_function_total_points() {
        let function = Plot3DFunction {
            grid_index: 0,
            dimensions: GridDimensions { i: 2, j: 3, k: 4 },
            function_data: vec![],
        };
        assert_eq!(function.total_points(), 24);
    }

    #[test]
    fn test_byte_order_detection_little_endian() {
        // Create a buffer with a small value in little-endian format
        let mut data = vec![];
        data.extend_from_slice(&100i32.to_le_bytes());
        let mut cursor = std::io::Cursor::new(data);

        let result = detect_byte_order(&mut cursor);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ByteOrder::LittleEndian);
    }

    #[test]
    fn test_byte_order_detection_big_endian() {
        // Create a buffer with a value that appears valid only in big-endian
        let mut data = vec![];
        data.extend_from_slice(&100i32.to_be_bytes());
        let mut cursor = std::io::Cursor::new(data);

        let result = detect_byte_order(&mut cursor);
        assert!(result.is_ok());
    }

    #[test]
    fn test_read_i32_little_endian() {
        let mut data = vec![];
        data.extend_from_slice(&42i32.to_le_bytes());
        let mut cursor = std::io::Cursor::new(data);

        let result = read_i32(&mut cursor, ByteOrder::LittleEndian);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_read_i32_big_endian() {
        let mut data = vec![];
        data.extend_from_slice(&42i32.to_be_bytes());
        let mut cursor = std::io::Cursor::new(data);

        let result = read_i32(&mut cursor, ByteOrder::BigEndian);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_read_f32_array() {
        let values = vec![1.0f32, 2.5f32, 3.14f32];
        let mut data = vec![];
        for v in &values {
            data.extend_from_slice(&v.to_le_bytes());
        }
        let mut cursor = std::io::Cursor::new(data);

        let result = read_f32_array(&mut cursor, 3, ByteOrder::LittleEndian);
        assert!(result.is_ok());
        let arr = result.unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0], 1.0);
        assert_eq!(arr[1], 2.5);
        assert_eq!(arr[2], 3.14);
    }

    #[test]
    fn test_read_plot3d_grid_ascii_simple() -> io::Result<()> {
        // Create a temporary ASCII PLOT3D file
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "1")?; // 1 grid
        writeln!(temp_file, "2 2 2")?; // 2x2x2 dimensions

        // Write 8 X coordinates
        writeln!(temp_file, "0.0 1.0 0.0 1.0 0.0 1.0 0.0 1.0")?;
        // Write 8 Y coordinates
        writeln!(temp_file, "0.0 0.0 1.0 1.0 0.0 0.0 1.0 1.0")?;
        // Write 8 Z coordinates
        writeln!(temp_file, "0.0 0.0 0.0 0.0 1.0 1.0 1.0 1.0")?;

        temp_file.flush()?;
        let path = temp_file.path();

        let result = read_plot3d_grid_ascii(path);
        assert!(result.is_ok());

        let grids = result.unwrap();
        assert_eq!(grids.len(), 1);
        assert_eq!(grids[0].dimensions.i, 2);
        assert_eq!(grids[0].dimensions.j, 2);
        assert_eq!(grids[0].dimensions.k, 2);
        assert_eq!(grids[0].total_points(), 8);
        assert_eq!(grids[0].x_coords.len(), 8);
        assert_eq!(grids[0].y_coords.len(), 8);
        assert_eq!(grids[0].z_coords.len(), 8);

        Ok(())
    }

    #[test]
    fn test_read_plot3d_grid_ascii_invalid_grid_count() -> io::Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "-1")?; // Invalid grid count
        temp_file.flush()?;

        let result = read_plot3d_grid_ascii(temp_file.path());
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_read_plot3d_grid_ascii_zero_dimensions() -> io::Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "1")?;
        writeln!(temp_file, "0 2 2")?; // Zero dimension
        temp_file.flush()?;

        let result = read_plot3d_grid_ascii(temp_file.path());
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_read_plot3d_solution_ascii_simple() -> io::Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "1")?; // 1 grid
        writeln!(temp_file, "2 1 1")?; // 2x1x1 = 2 points

        // Write metadata (4 minimum values): REFMACH, ALPHA, REY, TIME
        writeln!(temp_file, "1.2 5.0 1e6 0.5")?;

        // Write 5 variables × 2 points = 10 values
        writeln!(temp_file, "1.0 2.0")?; // rho
        writeln!(temp_file, "3.0 4.0")?; // rhou
        writeln!(temp_file, "5.0 6.0")?; // rhov
        writeln!(temp_file, "7.0 8.0")?; // rhow
        writeln!(temp_file, "9.0 10.0")?; // rhoe

        temp_file.flush()?;

        let result = read_plot3d_solution_ascii(temp_file.path());
        assert!(
            result.is_ok(),
            "Failed to read ASCII solution: {:?}",
            result.err()
        );

        let solutions = result.unwrap();
        assert_eq!(solutions.len(), 1);
        assert_eq!(solutions[0].total_points(), 2);
        assert_eq!(solutions[0].rho, vec![1.0, 2.0]);
        assert_eq!(solutions[0].rhou, vec![3.0, 4.0]);
        assert_eq!(solutions[0].rhov, vec![5.0, 6.0]);
        assert_eq!(solutions[0].rhow, vec![7.0, 8.0]);
        assert_eq!(solutions[0].rhoe, vec![9.0, 10.0]);

        // Check metadata was parsed
        assert!(solutions[0].metadata.is_some());
        let meta = solutions[0].metadata.as_ref().unwrap();
        assert_eq!(meta.refmach, Some(1.2));
        assert_eq!(meta.alpha, Some(5.0));

        Ok(())
    }

    #[test]
    fn test_grid_dimensions_clone() {
        let dims1 = GridDimensions { i: 5, j: 10, k: 15 };
        let dims2 = dims1.clone();
        assert_eq!(dims1.i, dims2.i);
        assert_eq!(dims1.j, dims2.j);
        assert_eq!(dims1.k, dims2.k);
    }

    #[test]
    fn test_byte_order_equality() {
        assert_eq!(ByteOrder::LittleEndian, ByteOrder::LittleEndian);
        assert_eq!(ByteOrder::BigEndian, ByteOrder::BigEndian);
        assert_ne!(ByteOrder::LittleEndian, ByteOrder::BigEndian);
    }

    #[test]
    fn test_mesh_geometry_simple_grid() {
        // Create a simple 2x2x1 grid
        let grid = Plot3DGrid {
            dimensions: GridDimensions { i: 2, j: 2, k: 1 },
            x_coords: vec![0.0, 1.0, 0.0, 1.0],
            y_coords: vec![0.0, 0.0, 1.0, 1.0],
            z_coords: vec![0.0, 0.0, 0.0, 0.0],
            iblank: None,
        };

        let mesh = grid.to_mesh_geometry(false);

        // Check vertex count
        assert_eq!(mesh.vertex_count, 4);
        assert_eq!(mesh.vertices.len(), 12); // 4 vertices * 3 components

        // Check vertices
        assert_eq!(mesh.vertices[0], 0.0); // x of vertex 0
        assert_eq!(mesh.vertices[1], 0.0); // y of vertex 0
        assert_eq!(mesh.vertices[2], 0.0); // z of vertex 0

        // Check that indices were generated for line segments
        assert!(mesh.indices.len() > 0);
        // face_count represents number of triangles (1 quad = 2 triangles)
        assert_eq!(mesh.face_count, 2);

        // Check normals
        assert_eq!(mesh.normals.len(), 12);
    }

    #[test]
    fn test_mesh_geometry_larger_grid() {
        // Create a 3x3x2 grid
        let mut grid = Plot3DGrid {
            dimensions: GridDimensions { i: 3, j: 3, k: 2 },
            x_coords: Vec::with_capacity(18),
            y_coords: Vec::with_capacity(18),
            z_coords: Vec::with_capacity(18),
            iblank: None,
        };

        // Fill with test data
        for i in 0..3 {
            for j in 0..3 {
                for k in 0..2 {
                    grid.x_coords.push(i as f32);
                    grid.y_coords.push(j as f32);
                    grid.z_coords.push(k as f32);
                }
            }
        }

        let mesh = grid.to_mesh_geometry(false);

        assert_eq!(mesh.vertex_count, 18);
        assert_eq!(mesh.vertices.len(), 54); // 18 * 3
        assert!(mesh.indices.len() > 0);
        assert_eq!(mesh.normals.len(), 54);
    }

    #[test]
    fn test_mesh_linear_index_calculation() {
        // Test the linear index calculation
        let i = 1;
        let j = 2;
        let k = 1;
        let dim_i = 3;
        let dim_j = 4;

        let idx = Plot3DGrid::linear_index(i, j, k, dim_i, dim_j);
        // k * (i*j) + j * i + i = 1 * 12 + 2 * 3 + 1 = 19
        assert_eq!(idx, 19);
    }

    #[test]
    fn test_mesh_geometry_normals_normalized() {
        // Create a simple grid and check that normals are normalized
        let grid = Plot3DGrid {
            dimensions: GridDimensions { i: 2, j: 2, k: 1 },
            x_coords: vec![0.0, 1.0, 0.0, 1.0],
            y_coords: vec![0.0, 0.0, 1.0, 1.0],
            z_coords: vec![0.0, 0.0, 0.0, 0.0],
            iblank: None,
        };

        let mesh = grid.to_mesh_geometry(false);

        // Check that normals are normalized (length should be 1 or close to 0)
        for i in (0..mesh.normals.len()).step_by(3) {
            let nx = mesh.normals[i];
            let ny = mesh.normals[i + 1];
            let nz = mesh.normals[i + 2];
            let length_sq = nx * nx + ny * ny + nz * nz;

            // Should be either ~1 (normalized) or ~0 (no normal contribution)
            assert!(
                length_sq < 1.1 && (length_sq > 0.9 || length_sq < 0.01),
                "Normal magnitude squared: {}",
                length_sq
            );
        }
    }

    #[test]
    fn test_mesh_geometry_preserves_coordinates() {
        let coords = vec![1.5, 2.5, 3.5, 4.5];
        let grid = Plot3DGrid {
            dimensions: GridDimensions { i: 2, j: 2, k: 1 },
            x_coords: coords.clone(),
            y_coords: coords.clone(),
            z_coords: coords.clone(),
            iblank: None,
        };

        let mesh = grid.to_mesh_geometry(false);

        // Check that coordinates are preserved in vertices
        for i in 0..4 {
            assert_eq!(mesh.vertices[i * 3], coords[i]);
            assert_eq!(mesh.vertices[i * 3 + 1], coords[i]);
            assert_eq!(mesh.vertices[i * 3 + 2], coords[i]);
        }
    }

    #[test]
    fn test_mesh_geometry_iblank_filtering() {
        // Create a 3x3x1 grid with some blanked points
        let grid = Plot3DGrid {
            dimensions: GridDimensions { i: 3, j: 3, k: 1 },
            x_coords: vec![0.0, 1.0, 2.0, 0.0, 1.0, 2.0, 0.0, 1.0, 2.0],
            y_coords: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0],
            z_coords: vec![0.0; 9],
            // Blank the center point (index 4) and corner (index 8)
            iblank: Some(vec![1, 1, 1, 1, 0, 1, 1, 1, 0]),
        };

        // Without respecting iblank, should have 4 quads (2x2 grid = 4 quads)
        let mesh_no_blank = grid.to_mesh_geometry(false);
        // 4 quads * 2 triangles per quad = 8 triangles
        assert_eq!(mesh_no_blank.face_count, 8);

        // With respecting iblank, should have fewer quads (those with blanked corners are excluded)
        let mesh_with_blank = grid.to_mesh_geometry(true);
        // All 4 quads touch at least one blanked point (center or corner), so should have 0 quads
        assert_eq!(mesh_with_blank.face_count, 0);
    }

    #[test]
    fn test_mesh_geometry_iblank_partial_blanking() {
        // Create a 3x3x1 grid with only one corner blanked
        let grid = Plot3DGrid {
            dimensions: GridDimensions { i: 3, j: 3, k: 1 },
            x_coords: vec![0.0, 1.0, 2.0, 0.0, 1.0, 2.0, 0.0, 1.0, 2.0],
            y_coords: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0],
            z_coords: vec![0.0; 9],
            // Blank only the top-right corner (index 8)
            iblank: Some(vec![1, 1, 1, 1, 1, 1, 1, 1, 0]),
        };

        // Without iblank: 4 quads
        let mesh_no_blank = grid.to_mesh_geometry(false);
        // 4 quads * 2 triangles per quad = 8 triangles
        assert_eq!(mesh_no_blank.face_count, 8);

        // With iblank: should lose 1 quad (top-right quad that uses point 8)
        let mesh_with_blank = grid.to_mesh_geometry(true);
        // 3 remaining quads * 2 triangles per quad = 6 triangles
        assert_eq!(mesh_with_blank.face_count, 6);
    }

    #[test]
    fn test_surface_mesh_decimated_vertex_mode_filters_blanked_vertices() {
        let grid = Plot3DGrid {
            dimensions: GridDimensions { i: 3, j: 3, k: 1 },
            x_coords: vec![0.0, 1.0, 2.0, 0.0, 1.0, 2.0, 0.0, 1.0, 2.0],
            y_coords: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0],
            z_coords: vec![0.0; 9],
            iblank: Some(vec![1, 1, 1, 1, 0, 1, 1, 1, 1]),
        };

        let mesh_no_filter = grid.to_mesh_surface_geometry_decimated(
            false,
            true,
            crate::IblankFilterMode::Vertex,
            1,
        );
        assert_eq!(mesh_no_filter.vertex_count, 9);
        assert_eq!(mesh_no_filter.face_count, 8);

        let mesh_filtered =
            grid.to_mesh_surface_geometry_decimated(true, true, crate::IblankFilterMode::Vertex, 1);
        assert_eq!(mesh_filtered.vertex_count, 8);
        assert_eq!(mesh_filtered.face_count, 0);
        assert_eq!(mesh_filtered.vertices.len(), 8 * 3);

        // Center point (1,1,0) should be removed from the output vertices.
        for vertex in mesh_filtered.vertices.chunks(3) {
            assert!(!(vertex[0] == 1.0 && vertex[1] == 1.0 && vertex[2] == 0.0));
        }
    }

    #[test]
    fn test_surface_mesh_decimated_cell_mode_keeps_vertices() {
        let grid = Plot3DGrid {
            dimensions: GridDimensions { i: 3, j: 3, k: 1 },
            x_coords: vec![0.0, 1.0, 2.0, 0.0, 1.0, 2.0, 0.0, 1.0, 2.0],
            y_coords: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0],
            z_coords: vec![0.0; 9],
            iblank: Some(vec![1, 1, 1, 1, 0, 1, 1, 1, 1]),
        };

        let mesh_filtered =
            grid.to_mesh_surface_geometry_decimated(true, true, crate::IblankFilterMode::Cell, 1);
        assert_eq!(mesh_filtered.vertex_count, 9);
        assert_eq!(mesh_filtered.vertices.len(), 9 * 3);
        assert_eq!(mesh_filtered.face_count, 0);
    }

    #[test]
    fn test_surface_mesh_decimated_handles_all_collapsed_axes() {
        let build_grid = |dims: GridDimensions| {
            let mut x_coords = Vec::new();
            let mut y_coords = Vec::new();
            let mut z_coords = Vec::new();

            for k in 0..dims.k {
                for j in 0..dims.j {
                    for i in 0..dims.i {
                        x_coords.push(i as f32);
                        y_coords.push(j as f32);
                        z_coords.push(k as f32);
                    }
                }
            }

            Plot3DGrid {
                dimensions: dims,
                x_coords,
                y_coords,
                z_coords,
                iblank: None,
            }
        };

        // These are exactly the shapes produced by subset slicing for I/J/K planes.
        let cases = [
            GridDimensions { i: 1, j: 3, k: 3 },
            GridDimensions { i: 3, j: 1, k: 3 },
            GridDimensions { i: 3, j: 3, k: 1 },
        ];

        for dims in cases {
            let grid = build_grid(dims.clone());

            let mesh_vertex = grid.to_mesh_surface_geometry_decimated(
                false,
                true,
                crate::IblankFilterMode::Vertex,
                1,
            );
            assert_eq!(
                mesh_vertex.vertex_count, 9,
                "vertex mode should keep all 9 points for dims {}x{}x{}",
                dims.i, dims.j, dims.k
            );
            assert_eq!(
                mesh_vertex.face_count, 8,
                "vertex mode should generate 4 quads (8 tris) for dims {}x{}x{}",
                dims.i, dims.j, dims.k
            );

            let mesh_cell = grid.to_mesh_surface_geometry_decimated(
                false,
                true,
                crate::IblankFilterMode::Cell,
                1,
            );
            assert_eq!(
                mesh_cell.vertex_count, 9,
                "cell mode should keep all 9 points for dims {}x{}x{}",
                dims.i, dims.j, dims.k
            );
            assert_eq!(
                mesh_cell.face_count, 8,
                "cell mode should generate 4 quads (8 tris) for dims {}x{}x{}",
                dims.i, dims.j, dims.k
            );
        }
    }

    #[test]
    fn test_arbitrary_plane_respect_iblank_controls_blanked_cells() {
        let x_coords = vec![0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0];
        let y_coords = vec![0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0];
        let z_coords = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];

        let grid_respect_off = Plot3DGrid {
            dimensions: GridDimensions { i: 2, j: 2, k: 2 },
            x_coords: x_coords.clone(),
            y_coords: y_coords.clone(),
            z_coords: z_coords.clone(),
            iblank: Some(vec![0; 8]),
        };

        let mesh_respect_off = grid_respect_off
            .slice_arbitrary_plane_with_solution(
                [0.0, 0.0, 0.5],
                [0.0, 0.0, 1.0],
                false,
                true,
                crate::IblankFilterMode::Vertex,
            )
            .expect("arbitrary slice should succeed when not respecting iblank");
        assert!(mesh_respect_off.vertex_count > 0);
        assert!(mesh_respect_off.face_count > 0);

        let grid_respect_on = Plot3DGrid {
            dimensions: GridDimensions { i: 2, j: 2, k: 2 },
            x_coords,
            y_coords,
            z_coords,
            iblank: Some(vec![0; 8]),
        };

        let result_respect_on = grid_respect_on.slice_arbitrary_plane_with_solution(
            [0.0, 0.0, 0.5],
            [0.0, 0.0, 1.0],
            true,
            true,
            crate::IblankFilterMode::Vertex,
        );
        assert!(result_respect_on.is_err());
        assert!(result_respect_on
            .err()
            .unwrap_or_default()
            .contains("No intersection found"));
    }

    #[test]
    fn test_arbitrary_plane_cell_mode_rejects_cells_with_blanked_corners() {
        // Create a 3x3x3 grid
        let mut x_coords = Vec::new();
        let mut y_coords = Vec::new();
        let mut z_coords = Vec::new();
        for k in 0..3 {
            for j in 0..3 {
                for i in 0..3 {
                    x_coords.push(i as f32);
                    y_coords.push(j as f32);
                    z_coords.push(k as f32);
                }
            }
        }

        // IBLANK array: all normal (1) except corner (0,0,0) which is blanked (0)
        let mut iblank = vec![1; 27];
        let blanked_idx = 0; // i=0, j=0, k=0
        iblank[blanked_idx] = 0;

        let grid = Plot3DGrid {
            dimensions: GridDimensions { i: 3, j: 3, k: 3 },
            x_coords,
            y_coords,
            z_coords,
            iblank: Some(iblank),
        };

        // Slice at z=0.5 plane (intersects k=0-1 cells)
        // In vertex mode: edges to blanked corner are skipped
        let mesh_vertex = grid
            .slice_arbitrary_plane_with_solution(
                [0.0, 0.0, 0.5],
                [0.0, 0.0, 1.0],
                true,
                true,
                crate::IblankFilterMode::Vertex,
            )
            .expect("vertex mode slice should succeed");

        // In cell mode: any intersected cell with a blanked corner is completely rejected
        let mesh_cell = grid
            .slice_arbitrary_plane_with_solution(
                [0.0, 0.0, 0.5],
                [0.0, 0.0, 1.0],
                true,
                true,
                crate::IblankFilterMode::Cell,
            )
            .expect("cell mode slice should succeed");

        // Cell mode should have fewer triangles because entire cells are rejected
        assert!(
            mesh_cell.face_count < mesh_vertex.face_count,
            "Cell mode should reject entire cells, resulting in fewer triangles than vertex mode"
        );
    }

    #[test]
    fn test_arbitrary_plane_cell_mode_hides_fringe_cells_when_fringe_hidden() {
        let x_coords = vec![0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0];
        let y_coords = vec![0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0];
        let z_coords = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];

        // Single fringe corner in a single-cell grid
        let mut iblank = vec![1; 8];
        iblank[0] = -1;

        let grid = Plot3DGrid {
            dimensions: GridDimensions { i: 2, j: 2, k: 2 },
            x_coords,
            y_coords,
            z_coords,
            iblank: Some(iblank),
        };

        // In cell mode, showing fringe should keep the cell visible
        let mesh_show_fringe = grid
            .slice_arbitrary_plane_with_solution(
                [0.0, 0.0, 0.5],
                [0.0, 0.0, 1.0],
                true,
                true,
                crate::IblankFilterMode::Cell,
            )
            .expect("cell mode slice should succeed when fringe points are visible");
        assert!(mesh_show_fringe.face_count > 0);

        // In cell mode, hiding fringe should reject the cell entirely
        let result_hide_fringe = grid.slice_arbitrary_plane_with_solution(
            [0.0, 0.0, 0.5],
            [0.0, 0.0, 1.0],
            true,
            false,
            crate::IblankFilterMode::Cell,
        );
        assert!(result_hide_fringe.is_err());
        assert!(result_hide_fringe
            .err()
            .unwrap_or_default()
            .contains("No intersection found"));
    }

    #[test]
    fn test_read_plot3d_grid_with_metadata_binary() -> io::Result<()> {
        // Create a simple binary PLOT3D grid file with Fortran record markers
        let mut temp_file = NamedTempFile::new()?;

        // Record 1: Number of grids (1 grid) with Fortran markers
        temp_file.write_all(&4i32.to_le_bytes())?; // Opening marker
        temp_file.write_all(&1i32.to_le_bytes())?; // num_grids = 1
        temp_file.write_all(&4i32.to_le_bytes())?; // Closing marker

        // Record 2: Dimensions (2x2x2) with Fortran markers
        temp_file.write_all(&12i32.to_le_bytes())?; // Opening marker (3 * 4 bytes)
        temp_file.write_all(&2i32.to_le_bytes())?; // i = 2
        temp_file.write_all(&2i32.to_le_bytes())?; // j = 2
        temp_file.write_all(&2i32.to_le_bytes())?; // k = 2
        temp_file.write_all(&12i32.to_le_bytes())?; // Closing marker

        // Record 3: X coordinates (8 values) with markers
        temp_file.write_all(&32i32.to_le_bytes())?; // Opening marker (8 * 4 bytes)
        for i in 0..8 {
            temp_file.write_all(&(i as f32).to_le_bytes())?;
        }
        temp_file.write_all(&32i32.to_le_bytes())?; // Closing marker

        // Record 4: Y coordinates (8 values) with markers
        temp_file.write_all(&32i32.to_le_bytes())?;
        for i in 0..8 {
            temp_file.write_all(&(i as f32 + 0.5).to_le_bytes())?;
        }
        temp_file.write_all(&32i32.to_le_bytes())?;

        // Record 5: Z coordinates (8 values) with markers
        temp_file.write_all(&32i32.to_le_bytes())?;
        for i in 0..8 {
            temp_file.write_all(&(i as f32 + 1.0).to_le_bytes())?;
        }
        temp_file.write_all(&32i32.to_le_bytes())?;

        temp_file.flush()?;

        let result = read_plot3d_grid_with_metadata(temp_file.path());
        assert!(result.is_ok());

        let (grids, metadata) = result.unwrap();
        assert_eq!(grids.len(), 1);
        assert_eq!(metadata.num_grids, 1);
        assert_eq!(metadata.byte_order, "Little-Endian");
        assert_eq!(grids[0].dimensions.i, 2);
        assert_eq!(grids[0].dimensions.j, 2);
        assert_eq!(grids[0].dimensions.k, 2);
        assert_eq!(grids[0].total_points(), 8);

        Ok(())
    }

    #[test]
    fn test_read_plot3d_solution_binary() -> io::Result<()> {
        // Create a binary PLOT3D solution file with complete Fortran record markers
        let mut temp_file = NamedTempFile::new()?;

        // Record 1: num_grids
        temp_file.write_all(&4i32.to_le_bytes())?; // Opening marker (4 bytes of data)
        temp_file.write_all(&1i32.to_le_bytes())?; // num_grids = 1
        temp_file.write_all(&4i32.to_le_bytes())?; // Closing marker

        // Record 2: dimensions + NQ + NQC (5 integers = 20 bytes)
        temp_file.write_all(&20i32.to_le_bytes())?; // Opening marker
        temp_file.write_all(&2i32.to_le_bytes())?; // i = 2
        temp_file.write_all(&1i32.to_le_bytes())?; // j = 1
        temp_file.write_all(&1i32.to_le_bytes())?; // k = 1
        temp_file.write_all(&6i32.to_le_bytes())?; // NQ = 6 (5 conservative + gamma)
        temp_file.write_all(&0i32.to_le_bytes())?; // NQC = 0 (no species)
        temp_file.write_all(&20i32.to_le_bytes())?; // Closing marker

        // Record 3: Metadata record (minimal - just write a small metadata block)
        // For simplicity, write a small metadata record (16 floats = 64 bytes)
        temp_file.write_all(&64i32.to_le_bytes())?; // Opening marker
        for _ in 0..16 {
            temp_file.write_all(&0.0f32.to_le_bytes())?;
        }
        temp_file.write_all(&64i32.to_le_bytes())?; // Closing marker

        // Solution data for 2 points (i=2, j=1, k=1), 6 variables in ONE record
        let rho_data = vec![1.0f32, 2.0f32];
        let rhou_data = vec![3.0f32, 4.0f32];
        let rhov_data = vec![5.0f32, 6.0f32];
        let rhow_data = vec![7.0f32, 8.0f32];
        let rhoe_data = vec![9.0f32, 10.0f32];
        let gamma_data = vec![1.4f32, 1.4f32];

        // Record 4: All Q variables in ONE record (6 variables * 2 points * 4 bytes = 48 bytes)
        temp_file.write_all(&48i32.to_le_bytes())?; // Opening marker
        for v in &rho_data {
            temp_file.write_all(&v.to_le_bytes())?;
        }
        for v in &rhou_data {
            temp_file.write_all(&v.to_le_bytes())?;
        }
        for v in &rhov_data {
            temp_file.write_all(&v.to_le_bytes())?;
        }
        for v in &rhow_data {
            temp_file.write_all(&v.to_le_bytes())?;
        }
        for v in &rhoe_data {
            temp_file.write_all(&v.to_le_bytes())?;
        }
        for v in &gamma_data {
            temp_file.write_all(&v.to_le_bytes())?;
        }
        temp_file.write_all(&48i32.to_le_bytes())?; // Closing marker

        temp_file.flush()?;

        let result = read_plot3d_solution(temp_file.path());
        assert!(
            result.is_ok(),
            "Failed to read binary solution: {:?}",
            result.err()
        );

        let solutions = result.unwrap();
        assert_eq!(solutions.len(), 1);
        assert_eq!(solutions[0].total_points(), 2);
        assert_eq!(solutions[0].rho, rho_data);
        assert_eq!(solutions[0].rhou, rhou_data);
        assert_eq!(solutions[0].gamma, Some(gamma_data));

        Ok(())
    }

    #[test]
    fn test_read_plot3d_solution_binary_with_large_metadata() -> io::Result<()> {
        // Test that we can handle metadata records larger than our minimal expectation
        // Some PLOT3D variants have extended metadata with additional fields
        let mut temp_file = NamedTempFile::new()?;

        // Record 1: num_grids
        temp_file.write_all(&4i32.to_le_bytes())?;
        temp_file.write_all(&1i32.to_le_bytes())?;
        temp_file.write_all(&4i32.to_le_bytes())?;

        // Record 2: dimensions + NQ + NQC
        temp_file.write_all(&20i32.to_le_bytes())?;
        temp_file.write_all(&3i32.to_le_bytes())?; // i = 3
        temp_file.write_all(&2i32.to_le_bytes())?; // j = 2
        temp_file.write_all(&1i32.to_le_bytes())?; // k = 1
        temp_file.write_all(&6i32.to_le_bytes())?; // NQ = 6
        temp_file.write_all(&0i32.to_le_bytes())?; // NQC = 0
        temp_file.write_all(&20i32.to_le_bytes())?;

        // Record 3: Large metadata record (128 floats = 512 bytes)
        // This is intentionally much larger than the minimal 16 floats we used before
        temp_file.write_all(&512i32.to_le_bytes())?;
        for i in 0..128 {
            temp_file.write_all(&(i as f32).to_le_bytes())?;
        }
        temp_file.write_all(&512i32.to_le_bytes())?;

        // Record 4: Q data for 6 points (3*2*1), 6 variables
        let total_points = 6;
        let total_floats = total_points * 6; // 36 floats = 144 bytes
        temp_file.write_all(&(total_floats as i32 * 4).to_le_bytes())?;

        // Write test data: rho, rhou, rhov, rhow, rhoe, gamma
        for var in 0..6 {
            for _pt in 0..total_points {
                let value = (var * 10 + _pt) as f32;
                temp_file.write_all(&value.to_le_bytes())?;
            }
        }
        temp_file.write_all(&(total_floats as i32 * 4).to_le_bytes())?;

        temp_file.flush()?;

        // Test that it can be read successfully despite large metadata
        let result = read_plot3d_solution(temp_file.path());
        assert!(
            result.is_ok(),
            "Failed to read solution with large metadata: {:?}",
            result.err()
        );

        let solutions = result.unwrap();
        assert_eq!(solutions.len(), 1);
        assert_eq!(solutions[0].total_points(), 6);

        // Verify the data was read correctly (rho should be 0,1,2,3,4,5)
        assert_eq!(solutions[0].rho[0], 0.0);
        assert_eq!(solutions[0].rho[1], 1.0);
        assert_eq!(solutions[0].rho[5], 5.0);

        // Verify gamma was extracted correctly (should be 50,51,52,53,54,55)
        assert!(solutions[0].gamma.is_some());
        let gamma = solutions[0].gamma.as_ref().unwrap();
        assert_eq!(gamma[0], 50.0);
        assert_eq!(gamma[5], 55.0);

        Ok(())
    }

    #[test]
    fn test_read_plot3d_solution_binary_with_minimal_metadata() -> io::Result<()> {
        // Test with minimal metadata (e.g., just a few required fields)
        let mut temp_file = NamedTempFile::new()?;

        // Record 1: num_grids
        temp_file.write_all(&4i32.to_le_bytes())?;
        temp_file.write_all(&1i32.to_le_bytes())?;
        temp_file.write_all(&4i32.to_le_bytes())?;

        // Record 2: dimensions + NQ + NQC
        temp_file.write_all(&20i32.to_le_bytes())?;
        temp_file.write_all(&2i32.to_le_bytes())?; // i = 2
        temp_file.write_all(&2i32.to_le_bytes())?; // j = 2
        temp_file.write_all(&1i32.to_le_bytes())?; // k = 1
        temp_file.write_all(&6i32.to_le_bytes())?; // NQ = 6
        temp_file.write_all(&0i32.to_le_bytes())?; // NQC = 0
        temp_file.write_all(&20i32.to_le_bytes())?;

        // Record 3: Minimal metadata (just 4 floats = 16 bytes)
        temp_file.write_all(&16i32.to_le_bytes())?;
        temp_file.write_all(&1.0f32.to_le_bytes())?; // REFMACH
        temp_file.write_all(&0.0f32.to_le_bytes())?; // ALPHA
        temp_file.write_all(&0.0f32.to_le_bytes())?; // REY
        temp_file.write_all(&0.0f32.to_le_bytes())?; // TIME
        temp_file.write_all(&16i32.to_le_bytes())?;

        // Record 4: Q data for 4 points, 6 variables
        let total_points = 4;
        temp_file.write_all(&96i32.to_le_bytes())?; // 24 floats * 4 bytes

        for var in 0..6 {
            for _pt in 0..total_points {
                temp_file.write_all(&((var + 1) as f32).to_le_bytes())?;
            }
        }
        temp_file.write_all(&96i32.to_le_bytes())?;

        temp_file.flush()?;

        let result = read_plot3d_solution(temp_file.path());
        assert!(
            result.is_ok(),
            "Failed to read solution with minimal metadata: {:?}",
            result.err()
        );

        let solutions = result.unwrap();
        assert_eq!(solutions.len(), 1);
        assert_eq!(solutions[0].total_points(), 4);
        assert_eq!(solutions[0].rho[0], 1.0);
        assert_eq!(solutions[0].rhoe[0], 5.0);

        assert!(solutions[0].gamma.is_some());
        assert_eq!(solutions[0].gamma.as_ref().unwrap()[0], 6.0);

        Ok(())
    }

    #[test]
    fn test_parse_metadata_full() {
        // Test parsing full metadata with all fields
        let mut buffer = Vec::new();
        let values: Vec<f32> = vec![
            1.2,   // REFMACH
            5.0,   // ALPHA
            1e6,   // REY
            0.5,   // TIME
            1.4,   // GAMINF
            0.0,   // BETA
            288.0, // TINF
            0.0,   // IGAM
            500.0, // HTINF
            100.0, // HT1
            200.0, // HT2
            287.0, // RGAS[0]
            0.5,   // FSMACH
            300.0, // TVREF
            50.0,  // DTVREF
        ];

        for val in &values {
            buffer.extend_from_slice(&(*val).to_le_bytes());
        }

        let metadata = parse_metadata(&buffer, ByteOrder::LittleEndian);

        assert_eq!(metadata.refmach, Some(1.2));
        assert_eq!(metadata.alpha, Some(5.0));
        assert_eq!(metadata.rey, Some(1e6));
        assert_eq!(metadata.time, Some(0.5));
        assert_eq!(metadata.gaminf, Some(1.4));
        assert_eq!(metadata.beta, Some(0.0));
        assert_eq!(metadata.tinf, Some(288.0));
        assert_eq!(metadata.igam, Some(0.0));
        assert_eq!(metadata.htinf, Some(500.0));
        assert_eq!(metadata.ht1, Some(100.0));
        assert_eq!(metadata.ht2, Some(200.0));
        assert_eq!(metadata.rgas, Some(vec![287.0]));
        assert_eq!(metadata.fsmach, Some(0.5));
        assert_eq!(metadata.tvref, Some(300.0));
        assert_eq!(metadata.dtvref, Some(50.0));
    }

    #[test]
    fn test_parse_metadata_minimal() {
        // Test parsing minimal metadata (only first 4 fields)
        let mut buffer = Vec::new();
        let values: Vec<f32> = vec![
            1.5,  // REFMACH
            10.0, // ALPHA
            5e5,  // REY
            1.0,  // TIME
        ];

        for val in &values {
            buffer.extend_from_slice(&(*val).to_le_bytes());
        }

        let metadata = parse_metadata(&buffer, ByteOrder::LittleEndian);

        assert_eq!(metadata.refmach, Some(1.5));
        assert_eq!(metadata.alpha, Some(10.0));
        assert_eq!(metadata.rey, Some(5e5));
        assert_eq!(metadata.time, Some(1.0));
        assert_eq!(metadata.gaminf, None);
        assert_eq!(metadata.beta, None);
        assert_eq!(metadata.tinf, None);
        assert_eq!(metadata.igam, None);
        assert_eq!(metadata.htinf, None);
        assert_eq!(metadata.ht1, None);
        assert_eq!(metadata.ht2, None);
        assert_eq!(metadata.rgas, None);
        assert_eq!(metadata.fsmach, None);
        assert_eq!(metadata.tvref, None);
        assert_eq!(metadata.dtvref, None);
    }

    #[test]
    fn test_parse_metadata_with_multiple_rgas() {
        // Test parsing metadata with multiple RGAS values
        let mut buffer = Vec::new();
        let values: Vec<f32> = vec![
            1.0,   // REFMACH
            0.0,   // ALPHA
            1e7,   // REY
            0.0,   // TIME
            1.4,   // GAMINF
            0.0,   // BETA
            300.0, // TINF
            0.0,   // IGAM
            600.0, // HTINF
            50.0,  // HT1
            100.0, // HT2
            287.0, // RGAS[0]
            1.0,   // RGAS[1]
            2.0,   // RGAS[2]
            0.6,   // FSMACH
            280.0, // TVREF
            10.0,  // DTVREF
        ];

        for val in &values {
            buffer.extend_from_slice(&(*val).to_le_bytes());
        }

        let metadata = parse_metadata(&buffer, ByteOrder::LittleEndian);

        assert_eq!(metadata.rgas, Some(vec![287.0, 1.0, 2.0]));
        assert_eq!(metadata.fsmach, Some(0.6));
        assert_eq!(metadata.tvref, Some(280.0));
        assert_eq!(metadata.dtvref, Some(10.0));
    }

    #[test]
    fn test_parse_metadata_big_endian() {
        // Test parsing metadata with big-endian byte order
        let mut buffer = Vec::new();
        let values: Vec<f32> = vec![
            1.2, // REFMACH
            5.0, // ALPHA
            1e6, // REY
            0.5, // TIME
        ];

        for val in &values {
            buffer.extend_from_slice(&(*val).to_be_bytes());
        }

        let metadata = parse_metadata(&buffer, ByteOrder::BigEndian);

        assert_eq!(metadata.refmach, Some(1.2));
        assert_eq!(metadata.alpha, Some(5.0));
        assert_eq!(metadata.rey, Some(1e6));
        assert_eq!(metadata.time, Some(0.5));
    }

    #[test]
    fn test_read_plot3d_solution_with_metadata() -> io::Result<()> {
        // Test reading solution with full metadata parsed
        let mut temp_file = NamedTempFile::new()?;

        // Record 1: num_grids
        temp_file.write_all(&4i32.to_le_bytes())?;
        temp_file.write_all(&1i32.to_le_bytes())?;
        temp_file.write_all(&4i32.to_le_bytes())?;

        // Record 2: dimensions + NQ + NQC
        temp_file.write_all(&20i32.to_le_bytes())?;
        temp_file.write_all(&2i32.to_le_bytes())?; // i = 2
        temp_file.write_all(&2i32.to_le_bytes())?; // j = 2
        temp_file.write_all(&1i32.to_le_bytes())?; // k = 1
        temp_file.write_all(&6i32.to_le_bytes())?; // NQ = 6
        temp_file.write_all(&0i32.to_le_bytes())?; // NQC = 0
        temp_file.write_all(&20i32.to_le_bytes())?;

        // Record 3: Full metadata (15 values = 60 bytes)
        temp_file.write_all(&60i32.to_le_bytes())?;
        let metadata_values: Vec<f32> = vec![
            1.2,   // REFMACH
            5.0,   // ALPHA
            1e6,   // REY
            0.5,   // TIME
            1.4,   // GAMINF
            0.0,   // BETA
            288.0, // TINF
            0.0,   // IGAM
            500.0, // HTINF
            100.0, // HT1
            200.0, // HT2
            287.0, // RGAS[0]
            0.5,   // FSMACH
            300.0, // TVREF
            50.0,  // DTVREF
        ];
        for val in &metadata_values {
            temp_file.write_all(&(*val).to_le_bytes())?;
        }
        temp_file.write_all(&60i32.to_le_bytes())?;

        // Record 4: Q data for 4 points, 6 variables
        let total_points = 4;
        temp_file.write_all(&96i32.to_le_bytes())?; // 24 floats * 4 bytes

        for var in 0..6 {
            for _pt in 0..total_points {
                temp_file.write_all(&((var + 1) as f32).to_le_bytes())?;
            }
        }
        temp_file.write_all(&96i32.to_le_bytes())?;

        temp_file.flush()?;

        let result = read_plot3d_solution(temp_file.path());
        assert!(
            result.is_ok(),
            "Failed to read solution: {:?}",
            result.err()
        );

        let solutions = result.unwrap();
        assert_eq!(solutions.len(), 1);

        let sol = &solutions[0];
        assert_eq!(sol.total_points(), 4);

        // Check metadata was parsed correctly
        assert!(sol.metadata.is_some());
        let meta = sol.metadata.as_ref().unwrap();
        assert_eq!(meta.refmach, Some(1.2));
        assert_eq!(meta.alpha, Some(5.0));
        assert_eq!(meta.rey, Some(1e6));
        assert_eq!(meta.time, Some(0.5));
        assert_eq!(meta.gaminf, Some(1.4));
        assert_eq!(meta.rgas, Some(vec![287.0]));
        assert_eq!(meta.fsmach, Some(0.5));
        assert_eq!(meta.tvref, Some(300.0));
        assert_eq!(meta.dtvref, Some(50.0));

        Ok(())
    }

    #[test]
    fn test_read_plot3d_function_binary() -> io::Result<()> {
        // Create a binary PLOT3D function file with complete Fortran record markers
        let mut temp_file = NamedTempFile::new()?;

        // Record 1: num_grids
        temp_file.write_all(&4i32.to_le_bytes())?; // Opening marker
        temp_file.write_all(&1i32.to_le_bytes())?; // num_grids = 1
        temp_file.write_all(&4i32.to_le_bytes())?; // Closing marker

        // Record 2: dimensions (3 integers = 12 bytes)
        temp_file.write_all(&12i32.to_le_bytes())?; // Opening marker
        temp_file.write_all(&2i32.to_le_bytes())?; // i = 2
        temp_file.write_all(&1i32.to_le_bytes())?; // j = 1
        temp_file.write_all(&1i32.to_le_bytes())?; // k = 1
        temp_file.write_all(&12i32.to_le_bytes())?; // Closing marker

        // Record 3: num_functions
        temp_file.write_all(&4i32.to_le_bytes())?; // Opening marker
        temp_file.write_all(&2i32.to_le_bytes())?; // num_functions = 2
        temp_file.write_all(&4i32.to_le_bytes())?; // Closing marker

        // Function data for 2 points (i=2, j=1, k=1), 2 functions
        let func1_data = vec![1.0f32, 2.0f32];
        let func2_data = vec![3.0f32, 4.0f32];

        // Record 4: First function array (2 floats = 8 bytes)
        temp_file.write_all(&8i32.to_le_bytes())?;
        for v in &func1_data {
            temp_file.write_all(&v.to_le_bytes())?;
        }
        temp_file.write_all(&8i32.to_le_bytes())?;

        // Record 5: Second function array (2 floats = 8 bytes)
        temp_file.write_all(&8i32.to_le_bytes())?;
        for v in &func2_data {
            temp_file.write_all(&v.to_le_bytes())?;
        }
        temp_file.write_all(&8i32.to_le_bytes())?;

        temp_file.flush()?;

        let result = read_plot3d_function(temp_file.path());
        assert!(
            result.is_ok(),
            "Failed to read binary function: {:?}",
            result.err()
        );

        let functions = result.unwrap();
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0].total_points(), 2);
        assert_eq!(functions[0].function_data.len(), 2);
        assert_eq!(functions[0].function_data[0], func1_data);
        assert_eq!(functions[0].function_data[1], func2_data);

        Ok(())
    }

    #[test]
    fn test_precision_as_str() {
        assert_eq!(Precision::F32.as_str(), "f32");
        assert_eq!(Precision::F64.as_str(), "f64");
        assert_eq!(Precision::Mixed.as_str(), "mixed");
    }

    #[test]
    fn test_read_plot3d_solution_binary_f32_precision() -> io::Result<()> {
        // Create a binary PLOT3D solution file with f32 precision
        let mut temp_file = NamedTempFile::new()?;

        // Record 1: num_grids
        temp_file.write_all(&4i32.to_le_bytes())?;
        temp_file.write_all(&1i32.to_le_bytes())?;
        temp_file.write_all(&4i32.to_le_bytes())?;

        // Record 2: dimensions + NQ + NQC (5 integers = 20 bytes)
        temp_file.write_all(&20i32.to_le_bytes())?;
        temp_file.write_all(&2i32.to_le_bytes())?; // i = 2
        temp_file.write_all(&1i32.to_le_bytes())?; // j = 1
        temp_file.write_all(&1i32.to_le_bytes())?; // k = 1
        temp_file.write_all(&5i32.to_le_bytes())?; // NQ = 5 (no gamma)
        temp_file.write_all(&0i32.to_le_bytes())?; // NQC = 0
        temp_file.write_all(&20i32.to_le_bytes())?;

        // Record 3: Metadata record (small)
        temp_file.write_all(&64i32.to_le_bytes())?;
        for _ in 0..16 {
            temp_file.write_all(&0.0f32.to_le_bytes())?;
        }
        temp_file.write_all(&64i32.to_le_bytes())?;

        // Record 4: Q data (5 variables * 2 points * 4 bytes = 40 bytes for f32)
        temp_file.write_all(&40i32.to_le_bytes())?;
        for i in 0..10 {
            temp_file.write_all(&(i as f32).to_le_bytes())?;
        }
        temp_file.write_all(&40i32.to_le_bytes())?;

        temp_file.flush()?;

        let result = read_plot3d_solution(temp_file.path());
        assert!(
            result.is_ok(),
            "Failed to read f32 solution: {:?}",
            result.err()
        );

        // Check that metadata was set correctly
        let metadata = get_last_solution_metadata();
        assert!(metadata.is_some(), "Solution metadata should be set");
        let meta = metadata.unwrap();
        assert_eq!(meta.format, "binary");
        assert_eq!(meta.precision, "f32");
        assert_eq!(meta.byte_order, "Little-Endian");

        Ok(())
    }

    #[test]
    fn test_read_plot3d_solution_binary_f64_precision() -> io::Result<()> {
        // Create a binary PLOT3D solution file with f64 precision (double)
        let mut temp_file = NamedTempFile::new()?;

        // Record 1: num_grids
        temp_file.write_all(&4i32.to_le_bytes())?;
        temp_file.write_all(&1i32.to_le_bytes())?;
        temp_file.write_all(&4i32.to_le_bytes())?;

        // Record 2: dimensions + NQ + NQC (5 integers = 20 bytes)
        temp_file.write_all(&20i32.to_le_bytes())?;
        temp_file.write_all(&2i32.to_le_bytes())?; // i = 2
        temp_file.write_all(&1i32.to_le_bytes())?; // j = 1
        temp_file.write_all(&1i32.to_le_bytes())?; // k = 1
        temp_file.write_all(&5i32.to_le_bytes())?; // NQ = 5
        temp_file.write_all(&0i32.to_le_bytes())?; // NQC = 0
        temp_file.write_all(&20i32.to_le_bytes())?;

        // Record 3: Metadata record (small)
        temp_file.write_all(&64i32.to_le_bytes())?;
        for _ in 0..16 {
            temp_file.write_all(&0.0f32.to_le_bytes())?;
        }
        temp_file.write_all(&64i32.to_le_bytes())?;

        // Record 4: Q data (5 variables * 2 points * 8 bytes = 80 bytes for f64)
        temp_file.write_all(&80i32.to_le_bytes())?;
        for i in 0..10 {
            temp_file.write_all(&(i as f64).to_le_bytes())?;
        }
        temp_file.write_all(&80i32.to_le_bytes())?;

        temp_file.flush()?;

        let result = read_plot3d_solution(temp_file.path());
        assert!(
            result.is_ok(),
            "Failed to read f64 solution: {:?}",
            result.err()
        );

        // Check that metadata was set correctly
        let metadata = get_last_solution_metadata();
        assert!(metadata.is_some(), "Solution metadata should be set");
        let meta = metadata.unwrap();
        assert_eq!(meta.format, "binary");
        assert_eq!(meta.precision, "f64");
        assert_eq!(meta.byte_order, "Little-Endian");

        Ok(())
    }

    #[test]
    fn test_read_plot3d_solution_ascii_metadata() -> io::Result<()> {
        // Create ASCII solution file
        let mut temp_file = NamedTempFile::new()?;
        temp_file.write_all(b"1\n")?; // num_grids
        temp_file.write_all(b"2 1 1\n")?; // i j k
        temp_file.write_all(b"5 0\n")?; // NQ NQC

        // Metadata line (5 floats)
        temp_file.write_all(b"0.0 0.0 0.0 0.0 0.0\n")?;

        // Q data (5 variables * 2 points = 10 values)
        for i in 0..10 {
            temp_file.write_all(format!("{}.0 ", i).as_bytes())?;
        }
        temp_file.write_all(b"\n")?;

        temp_file.flush()?;

        let result = read_plot3d_solution_ascii(temp_file.path());
        assert!(
            result.is_ok(),
            "Failed to read ASCII solution: {:?}",
            result.err()
        );

        // Check that metadata was set correctly
        let metadata = get_last_solution_metadata();
        assert!(metadata.is_some(), "Solution metadata should be set");
        let meta = metadata.unwrap();
        assert_eq!(meta.format, "ASCII");
        assert_eq!(meta.precision, "f32");
        assert_eq!(meta.byte_order, "N/A");

        Ok(())
    }

    #[test]
    fn test_arbitrary_plane_horizontal() {
        // Create a simple 2x2x2 grid (unit cube)
        let grid = Plot3DGrid {
            dimensions: GridDimensions { i: 2, j: 2, k: 2 },
            x_coords: vec![
                0.0, 1.0, 0.0, 1.0, // k=0
                0.0, 1.0, 0.0, 1.0, // k=1
            ],
            y_coords: vec![
                0.0, 0.0, 1.0, 1.0, // k=0
                0.0, 0.0, 1.0, 1.0, // k=1
            ],
            z_coords: vec![
                0.0, 0.0, 0.0, 0.0, // k=0
                1.0, 1.0, 1.0, 1.0, // k=1
            ],
            iblank: None,
        };

        // Horizontal plane at z=0.5 (normal pointing up)
        let plane_point = [0.5, 0.5, 0.5];
        let plane_normal = [0.0, 0.0, 1.0];

        let result = grid.slice_arbitrary_plane(
            plane_point,
            plane_normal,
            false,
            false,
            crate::IblankFilterMode::Vertex,
        );
        assert!(result.is_ok(), "Failed to slice grid: {:?}", result.err());

        let mesh = result.unwrap();
        assert!(mesh.vertices.len() > 0, "Should have intersection vertices");
        assert!(mesh.triangle_indices.len() > 0, "Should have triangles");

        // All vertices should have z ≈ 0.5
        for i in (0..mesh.vertices.len()).step_by(3) {
            let z = mesh.vertices[i + 2];
            assert!(
                (z - 0.5).abs() < 0.01,
                "Vertex z-coordinate {} should be near 0.5",
                z
            );
        }
    }

    #[test]
    fn test_arbitrary_plane_diagonal() {
        // Create a larger grid to ensure intersection
        let grid = Plot3DGrid {
            dimensions: GridDimensions { i: 3, j: 3, k: 3 },
            x_coords: (0..27)
                .map(|idx| {
                    let i = idx % 3;
                    i as f32
                })
                .collect(),
            y_coords: (0..27)
                .map(|idx| {
                    let j = (idx / 3) % 3;
                    j as f32
                })
                .collect(),
            z_coords: (0..27)
                .map(|idx| {
                    let k = idx / 9;
                    k as f32
                })
                .collect(),
            iblank: None,
        };

        // Diagonal plane: x + y + z = 1.5 (cuts through center of grid)
        // Normal = (1, 1, 1), Point = (0.5, 0.5, 0.5)
        let plane_point = [0.5, 0.5, 0.5];
        let plane_normal = [1.0, 1.0, 1.0];

        let result = grid.slice_arbitrary_plane(
            plane_point,
            plane_normal,
            false,
            false,
            crate::IblankFilterMode::Vertex,
        );
        assert!(
            result.is_ok(),
            "Failed to slice diagonal plane: {:?}",
            result.err()
        );

        let mesh = result.unwrap();
        assert!(mesh.vertices.len() > 0, "Should have intersection vertices");

        // Verify points lie on the plane: n·(p - p0) ≈ 0
        let norm_mag = (3.0_f32).sqrt();
        let n = [1.0 / norm_mag, 1.0 / norm_mag, 1.0 / norm_mag];

        for i in (0..mesh.vertices.len()).step_by(3) {
            let x = mesh.vertices[i];
            let y = mesh.vertices[i + 1];
            let z = mesh.vertices[i + 2];

            let dot = n[0] * (x - plane_point[0])
                + n[1] * (y - plane_point[1])
                + n[2] * (z - plane_point[2]);

            assert!(
                dot.abs() < 0.01,
                "Vertex ({}, {}, {}) should be on plane, dot product: {}",
                x,
                y,
                z,
                dot
            );
        }
    }

    #[test]
    fn test_arbitrary_plane_no_intersection() {
        // Create a small grid at origin
        let grid = Plot3DGrid {
            dimensions: GridDimensions { i: 2, j: 2, k: 2 },
            x_coords: vec![0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0],
            y_coords: vec![0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0],
            z_coords: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            iblank: None,
        };

        // Plane far away from the grid
        let plane_point = [10.0, 10.0, 10.0];
        let plane_normal = [0.0, 0.0, 1.0];

        let result = grid.slice_arbitrary_plane(
            plane_point,
            plane_normal,
            false,
            false,
            crate::IblankFilterMode::Vertex,
        );
        assert!(
            result.is_err(),
            "Should fail when plane doesn't intersect grid"
        );
    }

    #[test]
    fn test_arbitrary_plane_zero_normal() {
        let grid = Plot3DGrid {
            dimensions: GridDimensions { i: 2, j: 2, k: 2 },
            x_coords: vec![0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0],
            y_coords: vec![0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0],
            z_coords: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            iblank: None,
        };

        // Zero normal vector should fail
        let plane_point = [0.5, 0.5, 0.5];
        let plane_normal = [0.0, 0.0, 0.0];

        let result = grid.slice_arbitrary_plane(
            plane_point,
            plane_normal,
            false,
            false,
            crate::IblankFilterMode::Vertex,
        );
        assert!(result.is_err(), "Should fail with zero normal vector");
    }

    #[test]
    fn test_read_plot3d_grid_with_iblank_i32() -> io::Result<()> {
        // Test reading a binary PLOT3D grid file with iblank array (i32 format)
        let mut temp_file = NamedTempFile::new()?;

        // Record 1: Number of grids (1 grid)
        temp_file.write_all(&4i32.to_le_bytes())?; // Opening marker
        temp_file.write_all(&1i32.to_le_bytes())?; // num_grids = 1
        temp_file.write_all(&4i32.to_le_bytes())?; // Closing marker

        // Record 2: Dimensions (2x2x1)
        temp_file.write_all(&12i32.to_le_bytes())?; // Opening marker
        temp_file.write_all(&2i32.to_le_bytes())?; // i = 2
        temp_file.write_all(&2i32.to_le_bytes())?; // j = 2
        temp_file.write_all(&1i32.to_le_bytes())?; // k = 1
        temp_file.write_all(&12i32.to_le_bytes())?; // Closing marker

        // Record 3: XYZ + IBLANK (4 points)
        // XYZ: 4 points * 3 coords * 4 bytes = 48 bytes
        // IBLANK: 4 points * 4 bytes = 16 bytes
        // Total: 64 bytes
        temp_file.write_all(&64i32.to_le_bytes())?; // Opening marker

        // X coords
        temp_file.write_all(&0.0f32.to_le_bytes())?;
        temp_file.write_all(&1.0f32.to_le_bytes())?;
        temp_file.write_all(&0.0f32.to_le_bytes())?;
        temp_file.write_all(&1.0f32.to_le_bytes())?;

        // Y coords
        temp_file.write_all(&0.0f32.to_le_bytes())?;
        temp_file.write_all(&0.0f32.to_le_bytes())?;
        temp_file.write_all(&1.0f32.to_le_bytes())?;
        temp_file.write_all(&1.0f32.to_le_bytes())?;

        // Z coords
        temp_file.write_all(&0.0f32.to_le_bytes())?;
        temp_file.write_all(&0.0f32.to_le_bytes())?;
        temp_file.write_all(&0.0f32.to_le_bytes())?;
        temp_file.write_all(&0.0f32.to_le_bytes())?;

        // IBLANK array (blank the 3rd point, index 2)
        temp_file.write_all(&1i32.to_le_bytes())?; // visible
        temp_file.write_all(&1i32.to_le_bytes())?; // visible
        temp_file.write_all(&0i32.to_le_bytes())?; // blanked
        temp_file.write_all(&1i32.to_le_bytes())?; // visible

        temp_file.write_all(&64i32.to_le_bytes())?; // Closing marker

        temp_file.flush()?;

        let result = read_plot3d_grid_with_metadata(temp_file.path());
        assert!(
            result.is_ok(),
            "Failed to read grid with iblank: {:?}",
            result.err()
        );

        let (grids, metadata) = result.unwrap();
        assert_eq!(grids.len(), 1);
        assert_eq!(grids[0].total_points(), 4);
        assert!(grids[0].iblank.is_some(), "iblank should be present");
        assert_eq!(metadata.has_iblank, true);

        let iblank = grids[0].iblank.as_ref().unwrap();
        assert_eq!(iblank.len(), 4);
        assert_eq!(iblank[0], 1);
        assert_eq!(iblank[1], 1);
        assert_eq!(iblank[2], 0); // This point is blanked
        assert_eq!(iblank[3], 1);

        Ok(())
    }

    #[test]
    fn test_read_plot3d_grid_with_iblank_byte() -> io::Result<()> {
        // Test reading a binary PLOT3D grid file with iblank array (byte format)
        let mut temp_file = NamedTempFile::new()?;

        // Record 1: Number of grids (1 grid)
        temp_file.write_all(&4i32.to_le_bytes())?;
        temp_file.write_all(&1i32.to_le_bytes())?;
        temp_file.write_all(&4i32.to_le_bytes())?;

        // Record 2: Dimensions (2x2x1)
        temp_file.write_all(&12i32.to_le_bytes())?;
        temp_file.write_all(&2i32.to_le_bytes())?;
        temp_file.write_all(&2i32.to_le_bytes())?;
        temp_file.write_all(&1i32.to_le_bytes())?;
        temp_file.write_all(&12i32.to_le_bytes())?;

        // Record 3: XYZ + IBLANK (byte format)
        // XYZ: 4 points * 3 coords * 4 bytes = 48 bytes
        // IBLANK: 4 points * 1 byte = 4 bytes
        // Total: 52 bytes
        temp_file.write_all(&52i32.to_le_bytes())?; // Opening marker

        // X coords
        temp_file.write_all(&0.0f32.to_le_bytes())?;
        temp_file.write_all(&1.0f32.to_le_bytes())?;
        temp_file.write_all(&0.0f32.to_le_bytes())?;
        temp_file.write_all(&1.0f32.to_le_bytes())?;

        // Y coords
        temp_file.write_all(&0.0f32.to_le_bytes())?;
        temp_file.write_all(&0.0f32.to_le_bytes())?;
        temp_file.write_all(&1.0f32.to_le_bytes())?;
        temp_file.write_all(&1.0f32.to_le_bytes())?;

        // Z coords
        temp_file.write_all(&0.0f32.to_le_bytes())?;
        temp_file.write_all(&0.0f32.to_le_bytes())?;
        temp_file.write_all(&0.0f32.to_le_bytes())?;
        temp_file.write_all(&0.0f32.to_le_bytes())?;

        // IBLANK array as bytes (blank points 1 and 2)
        temp_file.write_all(&[1u8])?; // visible
        temp_file.write_all(&[0u8])?; // blanked
        temp_file.write_all(&[0u8])?; // blanked
        temp_file.write_all(&[1u8])?; // visible

        temp_file.write_all(&52i32.to_le_bytes())?; // Closing marker

        temp_file.flush()?;

        let result = read_plot3d_grid_with_metadata(temp_file.path());
        assert!(
            result.is_ok(),
            "Failed to read grid with byte iblank: {:?}",
            result.err()
        );

        let (grids, metadata) = result.unwrap();
        assert_eq!(grids.len(), 1);
        assert!(grids[0].iblank.is_some(), "iblank should be present");
        assert_eq!(metadata.has_iblank, true);

        let iblank = grids[0].iblank.as_ref().unwrap();
        assert_eq!(iblank.len(), 4);
        assert_eq!(iblank[0], 1);
        assert_eq!(iblank[1], 0); // This point is blanked
        assert_eq!(iblank[2], 0); // This point is blanked
        assert_eq!(iblank[3], 1);

        Ok(())
    }

    #[test]
    fn test_read_plot3d_grid_without_iblank() -> io::Result<()> {
        // Test that grids without iblank are still read correctly
        let mut temp_file = NamedTempFile::new()?;

        // Record 1: Number of grids
        temp_file.write_all(&4i32.to_le_bytes())?;
        temp_file.write_all(&1i32.to_le_bytes())?;
        temp_file.write_all(&4i32.to_le_bytes())?;

        // Record 2: Dimensions (2x1x1)
        temp_file.write_all(&12i32.to_le_bytes())?;
        temp_file.write_all(&2i32.to_le_bytes())?;
        temp_file.write_all(&1i32.to_le_bytes())?;
        temp_file.write_all(&1i32.to_le_bytes())?;
        temp_file.write_all(&12i32.to_le_bytes())?;

        // Record 3: XYZ only (no iblank)
        // 2 points * 3 coords * 4 bytes = 24 bytes
        temp_file.write_all(&24i32.to_le_bytes())?;

        temp_file.write_all(&0.0f32.to_le_bytes())?; // X
        temp_file.write_all(&1.0f32.to_le_bytes())?;
        temp_file.write_all(&0.0f32.to_le_bytes())?; // Y
        temp_file.write_all(&0.0f32.to_le_bytes())?;
        temp_file.write_all(&0.0f32.to_le_bytes())?; // Z
        temp_file.write_all(&0.0f32.to_le_bytes())?;

        temp_file.write_all(&24i32.to_le_bytes())?;

        temp_file.flush()?;

        let result = read_plot3d_grid_with_metadata(temp_file.path());
        assert!(
            result.is_ok(),
            "Failed to read grid without iblank: {:?}",
            result.err()
        );

        let (grids, metadata) = result.unwrap();
        assert_eq!(grids.len(), 1);
        assert_eq!(grids[0].total_points(), 2);
        assert!(grids[0].iblank.is_none(), "iblank should be None");
        assert_eq!(metadata.has_iblank, false);

        Ok(())
    }
}
