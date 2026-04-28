export interface Plot3DGrid {
    dimensions: { i: number; j: number; k: number };
    x_coords: number[];
    y_coords: number[];
    z_coords: number[];
    iblank?: number[]; // Optional blanking array (0=blanked, 1=normal, 2=wall, <0=fringe)
}

export interface Plot3DMetadata {
    fsmach?: number;  // Free stream Mach number
    refmach?: number; // Reference Mach number
    gaminf?: number;  // Gamma at infinity
    alpha?: number;   // Angle of attack (degrees)
    rey?: number;     // Reynolds number
    time?: number;    // Time value
}

export interface Plot3DSolution {
    grid_index: number;
    dimensions: { i: number; j: number; k: number };
    rho: number[];  // Density
    rhou: number[]; // Momentum X
    rhov: number[]; // Momentum Y
    rhow: number[]; // Momentum Z
    rhoe: number[]; // Energy
    gamma?: number[]; // Ratio of specific heats (always at Q[5], NQ=6+NQC+NQT)
    metadata?: Plot3DMetadata;
}

// New: Metadata types for cached grids/solutions (no coordinate arrays)
export interface GridMetadata {
    id: string;
    file_path: string;
    file_name: string;
    grid_index: number;
    dimensions: { i: number; j: number; k: number };
    has_iblank: boolean;
    has_solution: boolean;
}

export interface SolutionMetadata {
    id: string;
    file_path: string;
    file_name: string;
    grid_index: number;
    dimensions: { i: number; j: number; k: number };
}

// Contour-related types
export interface ContourSettings {
    enabled: boolean;
    level: number; // 0.0 to 1.0 normalized
}

export interface IsoSurfaceGeometry {
    gridId: string;
    positions: Float32Array;
    normals: Float32Array;
    indices: Uint32Array;
}

export interface ContourLineGeometry {
    sliceId: string;
    positions: Float32Array; // line segments as pairs of points
}
