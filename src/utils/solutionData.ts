/**
 * Utilities for computing derived quantities from PLOT3D solution data
 * PLOT3D solution variables: rho, rhou, rhov, rhow, rhoe
 * (Density, Momentum X, Momentum Y, Momentum Z, Energy)
 */

import type { Plot3DSolution } from "../types/plot3d";
import { DEFAULT_GAMMA } from "./constants";

/**
 * Supported scalar fields for visualization
 */
export type ScalarField =
    | 'none'
    | 'density'
    | 'velocity_magnitude'
    | 'momentum_x'
    | 'momentum_y'
    | 'momentum_z'
    | 'pressure'
    | 'energy'
    | 'u_velocity'
    | 'v_velocity'
    | 'w_velocity'
    // Density family
    | 'normalized_density'
    | 'stagnation_density'
    | 'normalized_stagnation_density'
    | 'log_normalized_density'
    // Pressure family
    | 'normalized_pressure'
    | 'stagnation_pressure'
    | 'normalized_stagnation_pressure'
    | 'pressure_coefficient'
    | 'stagnation_pressure_coefficient'
    | 'pitot_pressure'
    | 'pitot_pressure_ratio'
    | 'dynamic_pressure'
    | 'log_normalized_pressure'
    // Temperature family
    | 'temperature'
    | 'normalized_temperature'
    | 'stagnation_temperature'
    | 'normalized_stagnation_temperature'
    | 'log_normalized_temperature'
    // Enthalpy family
    | 'enthalpy'
    | 'normalized_enthalpy'
    | 'stagnation_enthalpy'
    | 'normalized_stagnation_enthalpy'
    // Energy family
    | 'internal_energy'
    | 'normalized_internal_energy'
    | 'stagnation_energy'
    | 'normalized_stagnation_energy'
    | 'kinetic_energy'
    | 'normalized_kinetic_energy'
    // Velocity / flow family
    | 'mach_number'
    | 'speed_of_sound'
    | 'cross_flow_velocity'
    | 'normalized_2d_stream_function'
    | 'velocity_divergence'
    // Entropy family
    | 'entropy'
    | 'entropy_measure_s1'
    // Vorticity / helicity family (require grid coords — computed by backend)
    | 'vorticity_x'
    | 'vorticity_y'
    | 'vorticity_z'
    | 'vorticity_magnitude'
    | 'swirl'
    | 'velocity_cross_vorticity_magnitude'
    | 'helicity_density'
    | 'relative_helicity'
    | 'filtered_relative_helicity'
    // Shock / gradient family (require grid coords — computed by backend)
    | 'shock_function_pressure_gradient'
    | 'filtered_shock_function'
    | 'pressure_gradient_magnitude'
    | 'density_gradient_magnitude';

export interface ScalarFieldInfo {
    field: ScalarField;
    name: string;
    unit: string;
    description: string;
}

export const SCALAR_FIELDS: ScalarFieldInfo[] = [
    { field: 'none', name: 'Grid ID', unit: '', description: 'Color by grid number (no solution visualization)' },
    { field: 'density', name: 'Density', unit: 'ρ', description: 'Fluid density' },
    { field: 'pressure', name: 'Pressure', unit: 'p', description: 'Static pressure' },
    { field: 'velocity_magnitude', name: 'Velocity Magnitude', unit: '|V|', description: 'Total velocity magnitude √(u²+v²+w²)' },
    { field: 'u_velocity', name: 'U Velocity', unit: 'u', description: 'X-component of velocity' },
    { field: 'v_velocity', name: 'V Velocity', unit: 'v', description: 'Y-component of velocity' },
    { field: 'w_velocity', name: 'W Velocity', unit: 'w', description: 'Z-component of velocity' },
    { field: 'momentum_x', name: 'Momentum X', unit: 'ρu', description: 'X-component of momentum' },
    { field: 'momentum_y', name: 'Momentum Y', unit: 'ρv', description: 'Y-component of momentum' },
    { field: 'momentum_z', name: 'Momentum Z', unit: 'ρw', description: 'Z-component of momentum' },
    { field: 'energy', name: 'Total Energy', unit: 'ρe', description: 'Total energy per unit volume (Q5)' },
    // Density family
    { field: 'normalized_density', name: 'Normalized Density', unit: 'ρ/ρ∞', description: 'Density ratio ρ/ρ∞ (= ρ in non-dimensional form)' },
    { field: 'stagnation_density', name: 'Stagnation Density', unit: 'ρ₀', description: 'Isentropic stagnation density' },
    { field: 'normalized_stagnation_density', name: 'Normalized Stagnation Density', unit: 'ρ₀/ρ∞', description: 'Stagnation density ratio' },
    { field: 'log_normalized_density', name: 'Log Normalized Density', unit: 'ln(ρ/ρ∞)', description: 'Natural log of normalized density' },
    // Pressure family
    { field: 'normalized_pressure', name: 'Normalized Pressure', unit: 'p/p∞', description: 'Pressure ratio p/p∞' },
    { field: 'stagnation_pressure', name: 'Stagnation Pressure', unit: 'p₀', description: 'Isentropic stagnation pressure' },
    { field: 'normalized_stagnation_pressure', name: 'Normalized Stagnation Pressure', unit: 'p₀/p∞', description: 'Stagnation pressure ratio' },
    { field: 'pressure_coefficient', name: 'Pressure Coefficient', unit: 'Cp', description: 'Cp = (p−p∞) / (½ρ∞V∞²)' },
    { field: 'stagnation_pressure_coefficient', name: 'Stagnation Pressure Coefficient', unit: 'Cp₀', description: 'Stagnation pressure coefficient' },
    { field: 'pitot_pressure', name: 'Pitot Pressure', unit: 'pp', description: 'Pitot (impact) pressure; Rayleigh formula for M≥1' },
    { field: 'pitot_pressure_ratio', name: 'Pitot Pressure Ratio', unit: 'pp/p∞', description: 'Pitot pressure normalized by freestream static pressure' },
    { field: 'dynamic_pressure', name: 'Dynamic Pressure', unit: 'q', description: 'q = ½ρV²' },
    { field: 'log_normalized_pressure', name: 'Log Normalized Pressure', unit: 'ln(p/p∞)', description: 'Natural log of normalized pressure' },
    // Temperature family
    { field: 'temperature', name: 'Temperature', unit: 'T', description: 'Static temperature T = p/(ρR)' },
    { field: 'normalized_temperature', name: 'Normalized Temperature', unit: 'T/T∞', description: 'Temperature ratio T/T∞' },
    { field: 'stagnation_temperature', name: 'Stagnation Temperature', unit: 'T₀', description: 'Stagnation temperature' },
    { field: 'normalized_stagnation_temperature', name: 'Normalized Stagnation Temperature', unit: 'T₀/T∞', description: 'Stagnation temperature ratio' },
    { field: 'log_normalized_temperature', name: 'Log Normalized Temperature', unit: 'ln(T/T∞)', description: 'Natural log of normalized temperature' },
    // Enthalpy family
    { field: 'enthalpy', name: 'Enthalpy', unit: 'h', description: 'Static enthalpy h = γeᵢ' },
    { field: 'normalized_enthalpy', name: 'Normalized Enthalpy', unit: 'h/h∞', description: 'Enthalpy ratio h/h∞' },
    { field: 'stagnation_enthalpy', name: 'Stagnation Enthalpy', unit: 'h₀', description: 'Total enthalpy h₀ = e₀ + p/ρ' },
    { field: 'normalized_stagnation_enthalpy', name: 'Normalized Stagnation Enthalpy', unit: 'h₀/h₀∞', description: 'Stagnation enthalpy ratio' },
    // Energy family
    { field: 'internal_energy', name: 'Internal Energy', unit: 'eᵢ', description: 'Specific internal energy eᵢ = e₀ − ½V²' },
    { field: 'normalized_internal_energy', name: 'Normalized Internal Energy', unit: 'eᵢ/eᵢ∞', description: 'Internal energy ratio' },
    { field: 'stagnation_energy', name: 'Stagnation Energy', unit: 'e₀', description: 'Specific total energy per unit mass e₀ = Q5/ρ' },
    { field: 'normalized_stagnation_energy', name: 'Normalized Stagnation Energy', unit: 'e₀/e₀∞', description: 'Stagnation energy ratio' },
    { field: 'kinetic_energy', name: 'Kinetic Energy', unit: 'eₖ', description: 'Specific kinetic energy eₖ = ½V²' },
    { field: 'normalized_kinetic_energy', name: 'Normalized Kinetic Energy', unit: 'eₖ/eₖ∞', description: 'Kinetic energy ratio' },
    // Velocity / flow family
    { field: 'mach_number', name: 'Mach Number', unit: 'M', description: 'Local Mach number M = |V|/c' },
    { field: 'speed_of_sound', name: 'Speed of Sound', unit: 'c', description: 'Local speed of sound c = √(γp/ρ)' },
    { field: 'cross_flow_velocity', name: 'Cross-Flow Velocity', unit: 'Vcf', description: 'Cross-flow speed √(v²+w²)' },
    { field: 'normalized_2d_stream_function', name: '2D Stream Function', unit: 'ψ/M∞', description: 'Normalized 2D stream function (requires grid; zero without grid data)' },
    { field: 'velocity_divergence', name: 'Velocity Divergence', unit: '∇·V', description: 'Divergence of velocity field (requires grid)' },
    // Entropy family
    { field: 'entropy', name: 'Entropy', unit: 's', description: 'Entropy s = cᵥ·ln[(p/p∞)/(ρ/ρ∞)^γ]' },
    { field: 'entropy_measure_s1', name: 'Entropy Measure s₁', unit: 's₁', description: 'Isentropic entropy ratio s₁ = (p/p∞)/(ρ/ρ∞)^γ − 1' },
    // Vorticity / helicity family
    { field: 'vorticity_x', name: 'Vorticity X', unit: 'ω₁', description: 'X-component of vorticity ∂w/∂y − ∂v/∂z (requires grid)' },
    { field: 'vorticity_y', name: 'Vorticity Y', unit: 'ω₂', description: 'Y-component of vorticity ∂u/∂z − ∂w/∂x (requires grid)' },
    { field: 'vorticity_z', name: 'Vorticity Z', unit: 'ω₃', description: 'Z-component of vorticity ∂v/∂x − ∂u/∂y (requires grid)' },
    { field: 'vorticity_magnitude', name: 'Vorticity Magnitude', unit: '|ω|', description: 'Vorticity magnitude |ω| (requires grid)' },
    { field: 'swirl', name: 'Swirl', unit: '', description: 'Swirl (ω·V)/(ρV²) (requires grid)' },
    { field: 'velocity_cross_vorticity_magnitude', name: '|V × ω|', unit: '', description: 'Magnitude of velocity cross vorticity (requires grid)' },
    { field: 'helicity_density', name: 'Helicity Density', unit: 'V·ω', description: 'Helicity density V·ω (requires grid)' },
    { field: 'relative_helicity', name: 'Relative Helicity', unit: 'cos φ', description: 'Relative helicity cos(φ) = V·ω/(|V||ω|) (requires grid)' },
    { field: 'filtered_relative_helicity', name: 'Filtered Relative Helicity', unit: '', description: 'Relative helicity where |V·ω| ≥ 0.1V∞²; else 0 (requires grid)' },
    // Shock / gradient family
    { field: 'shock_function_pressure_gradient', name: 'Shock Function', unit: '', description: 'Mach component in direction of ∇p (requires grid)' },
    { field: 'filtered_shock_function', name: 'Filtered Shock Function', unit: '', description: 'Shock function where |∇p| ≥ 0.1; else 0 (requires grid)' },
    { field: 'pressure_gradient_magnitude', name: 'Pressure Gradient |∇p|', unit: '|∇p|', description: 'Pressure gradient magnitude (requires grid)' },
    { field: 'density_gradient_magnitude', name: 'Density Gradient |∇ρ|', unit: '|∇ρ|', description: 'Density gradient magnitude / schlieren (requires grid)' },
];

/**
 * Compute velocity magnitude from momentum components
 */
function computeVelocityMagnitude(rho: number, rhou: number, rhov: number, rhow: number): number {
    if (rho <= 0) return 0;

    const u = rhou / rho;
    const v = rhov / rho;
    const w = rhow / rho;
    return Math.sqrt(u * u + v * v + w * w);
}

/**
 * Compute pressure from conservative variables
 */
function computePressure(
    rho: number,
    rhou: number,
    rhov: number,
    rhow: number,
    rhoe: number,
    gamma: number
): number {
    if (rho <= 0) return 0;

    const u = rhou / rho;
    const v = rhov / rho;
    const w = rhow / rho;
    const kinetic_energy = 0.5 * rho * (u * u + v * v + w * w);
    const internal_energy = rhoe - kinetic_energy;
    return (gamma - 1) * internal_energy;
}

/**
 * Compute a scalar field from solution data.
 *
 * Derivative-based fields (vorticity, divergence, gradients, helicity,
 * shock functions, 2D stream function) require grid coordinates and cannot
 * be computed here; those cases return a zero array.
 */
export function computeScalarField(solution: Plot3DSolution, field: ScalarField): Float32Array {
    const n = solution.rho.length;
    const result = new Float32Array(n);

    // Freestream reference (PLOT3D non-dimensional: ρ∞=1, c∞=1, p∞=1/γ)
    const gamma = DEFAULT_GAMMA;
    const minf: number = (solution.metadata?.fsmach ?? solution.metadata?.refmach ?? 1.0) as number;
    const pInf = 1.0 / gamma;
    const vinfSq = minf * minf;
    const dynInf = vinfSq > 0 ? 0.5 * vinfSq : 1.0;

    // Per-point helper: static pressure at index i
    const pressure = (i: number): number => {
        const rho = solution.rho[i];
        if (rho <= 0) return 0;
        const g = solution.gamma ? solution.gamma[i] : DEFAULT_GAMMA;
        const u = solution.rhou[i] / rho;
        const v = solution.rhov[i] / rho;
        const w = solution.rhow[i] / rho;
        return Math.max(0, (g - 1) * (solution.rhoe[i] - 0.5 * rho * (u * u + v * v + w * w)));
    };

    switch (field) {
        case 'none':
            return new Float32Array(n);

        case 'density':
            return new Float32Array(solution.rho);

        case 'normalized_density':
            return new Float32Array(solution.rho); // ρ/ρ∞ = ρ (ρ∞=1)

        case 'log_normalized_density':
            for (let i = 0; i < n; i++) {
                result[i] = solution.rho[i] > 0 ? Math.log(solution.rho[i]) : 0;
            }
            return result;

        case 'stagnation_density':
        case 'normalized_stagnation_density': {
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                if (rho <= 0) continue;
                const g = solution.gamma ? solution.gamma[i] : DEFAULT_GAMMA;
                const p = pressure(i);
                const c2 = Math.max(0, g * p / rho);
                const u = solution.rhou[i] / rho;
                const v = solution.rhov[i] / rho;
                const w = solution.rhow[i] / rho;
                const v2 = u * u + v * v + w * w;
                const c = Math.sqrt(c2);
                const mach = c > 0 ? Math.sqrt(v2) / c : 0;
                const base = 1 + (g - 1) / 2 * mach * mach;
                result[i] = rho * Math.pow(base, 1 / (g - 1));
            }
            return result;
        }

        case 'pressure':
            for (let i = 0; i < n; i++) result[i] = pressure(i);
            return result;

        case 'normalized_pressure':
            for (let i = 0; i < n; i++) {
                const g = solution.gamma ? solution.gamma[i] : DEFAULT_GAMMA;
                result[i] = pressure(i) * g;
            }
            return result;

        case 'stagnation_pressure':
        case 'normalized_stagnation_pressure': {
            const normalize = field === 'normalized_stagnation_pressure';
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                if (rho <= 0) continue;
                const g = solution.gamma ? solution.gamma[i] : DEFAULT_GAMMA;
                const p = pressure(i);
                const u = solution.rhou[i] / rho;
                const v = solution.rhov[i] / rho;
                const w = solution.rhow[i] / rho;
                const v2 = u * u + v * v + w * w;
                const c = Math.sqrt(Math.max(0, g * p / rho));
                const mach = c > 0 ? Math.sqrt(v2) / c : 0;
                const base = 1 + (g - 1) / 2 * mach * mach;
                const p0 = p * Math.pow(base, g / (g - 1));
                result[i] = normalize ? p0 * g : p0;
            }
            return result;
        }

        case 'pressure_coefficient':
            for (let i = 0; i < n; i++) {
                result[i] = (pressure(i) - pInf) / dynInf;
            }
            return result;

        case 'stagnation_pressure_coefficient': {
            const g0Inf = 1.0 + (gamma - 1) / 2 * minf * minf;
            const p0Inf = pInf * Math.pow(g0Inf, gamma / (gamma - 1));
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                if (rho <= 0) continue;
                const g = solution.gamma ? solution.gamma[i] : DEFAULT_GAMMA;
                const p = pressure(i);
                const u = solution.rhou[i] / rho;
                const v = solution.rhov[i] / rho;
                const w = solution.rhow[i] / rho;
                const v2 = u * u + v * v + w * w;
                const c = Math.sqrt(Math.max(0, g * p / rho));
                const mach = c > 0 ? Math.sqrt(v2) / c : 0;
                const base = 1 + (g - 1) / 2 * mach * mach;
                const p0 = p * Math.pow(base, g / (g - 1));
                result[i] = (p0 - p0Inf) / dynInf;
            }
            return result;
        }

        case 'pitot_pressure':
        case 'pitot_pressure_ratio': {
            const normalize = field === 'pitot_pressure_ratio';
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                if (rho <= 0) continue;
                const g = solution.gamma ? solution.gamma[i] : DEFAULT_GAMMA;
                const p = pressure(i);
                const u = solution.rhou[i] / rho;
                const v = solution.rhov[i] / rho;
                const w = solution.rhow[i] / rho;
                const v2 = u * u + v * v + w * w;
                const c = Math.sqrt(Math.max(0, g * p / rho));
                const mach = c > 0 ? Math.sqrt(v2) / c : 0;
                let pp: number;
                if (mach < 1) {
                    const base = 1 + (g - 1) / 2 * mach * mach;
                    pp = p * Math.pow(base, g / (g - 1));
                } else {
                    const m2 = mach * mach;
                    const gm1 = g - 1; const gp1 = g + 1;
                    const numer = Math.pow(gp1 / 2 * m2, g / gm1);
                    const denomBase = (2 * g * m2 - gm1) / gp1;
                    const denom = denomBase > 0 ? Math.pow(denomBase, 1 / gm1) : Number.EPSILON;
                    pp = p * numer / denom;
                }
                result[i] = normalize ? pp * g : pp;
            }
            return result;
        }

        case 'dynamic_pressure':
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                if (rho <= 0) continue;
                const u = solution.rhou[i] / rho;
                const v = solution.rhov[i] / rho;
                const w = solution.rhow[i] / rho;
                result[i] = 0.5 * rho * (u * u + v * v + w * w);
            }
            return result;

        case 'log_normalized_pressure':
            for (let i = 0; i < n; i++) {
                const g = solution.gamma ? solution.gamma[i] : DEFAULT_GAMMA;
                const np = pressure(i) * g;
                result[i] = np > 0 ? Math.log(np) : 0;
            }
            return result;

        case 'temperature':
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                result[i] = rho > 0 ? pressure(i) / rho : 0;
            }
            return result;

        case 'normalized_temperature':
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                if (rho <= 0) continue;
                const g = solution.gamma ? solution.gamma[i] : DEFAULT_GAMMA;
                result[i] = pressure(i) / rho * g;
            }
            return result;

        case 'stagnation_temperature':
        case 'normalized_stagnation_temperature': {
            const normalize = field === 'normalized_stagnation_temperature';
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                if (rho <= 0) continue;
                const g = solution.gamma ? solution.gamma[i] : DEFAULT_GAMMA;
                const p = pressure(i);
                const u = solution.rhou[i] / rho;
                const v = solution.rhov[i] / rho;
                const w = solution.rhow[i] / rho;
                const v2 = u * u + v * v + w * w;
                const c = Math.sqrt(Math.max(0, g * p / rho));
                const mach = c > 0 ? Math.sqrt(v2) / c : 0;
                const base = 1 + (g - 1) / 2 * mach * mach;
                const t0 = (p / rho) * base;
                result[i] = normalize ? t0 * g : t0;
            }
            return result;
        }

        case 'log_normalized_temperature':
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                if (rho <= 0) continue;
                const g = solution.gamma ? solution.gamma[i] : DEFAULT_GAMMA;
                const nt = pressure(i) / rho * g;
                result[i] = nt > 0 ? Math.log(nt) : 0;
            }
            return result;

        case 'enthalpy':
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                if (rho <= 0) continue;
                const g = solution.gamma ? solution.gamma[i] : DEFAULT_GAMMA;
                const u = solution.rhou[i] / rho;
                const v = solution.rhov[i] / rho;
                const w = solution.rhow[i] / rho;
                const ei = solution.rhoe[i] / rho - 0.5 * (u * u + v * v + w * w);
                result[i] = g * ei;
            }
            return result;

        case 'normalized_enthalpy':
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                if (rho <= 0) continue;
                const g = solution.gamma ? solution.gamma[i] : DEFAULT_GAMMA;
                const u = solution.rhou[i] / rho;
                const v = solution.rhov[i] / rho;
                const w = solution.rhow[i] / rho;
                const ei = solution.rhoe[i] / rho - 0.5 * (u * u + v * v + w * w);
                result[i] = g * ei * (g - 1);
            }
            return result;

        case 'stagnation_enthalpy':
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                if (rho <= 0) continue;
                const e0 = solution.rhoe[i] / rho;
                result[i] = e0 + pressure(i) / rho;
            }
            return result;

        case 'normalized_stagnation_enthalpy': {
            const h0inf = 1 / (gamma - 1) + 0.5 * vinfSq || 1.0;
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                if (rho <= 0) continue;
                result[i] = (solution.rhoe[i] / rho + pressure(i) / rho) / h0inf;
            }
            return result;
        }

        case 'internal_energy':
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                if (rho <= 0) continue;
                const u = solution.rhou[i] / rho;
                const v = solution.rhov[i] / rho;
                const w = solution.rhow[i] / rho;
                result[i] = solution.rhoe[i] / rho - 0.5 * (u * u + v * v + w * w);
            }
            return result;

        case 'normalized_internal_energy':
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                if (rho <= 0) continue;
                const g = solution.gamma ? solution.gamma[i] : DEFAULT_GAMMA;
                const u = solution.rhou[i] / rho;
                const v = solution.rhov[i] / rho;
                const w = solution.rhow[i] / rho;
                const ei = solution.rhoe[i] / rho - 0.5 * (u * u + v * v + w * w);
                result[i] = ei * g * (g - 1);
            }
            return result;

        case 'stagnation_energy':
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                result[i] = rho > 0 ? solution.rhoe[i] / rho : 0;
            }
            return result;

        case 'normalized_stagnation_energy': {
            const eiInf = 1 / (gamma * (gamma - 1));
            const e0inf = Math.max(Number.EPSILON, eiInf + 0.5 * vinfSq);
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                result[i] = rho > 0 ? (solution.rhoe[i] / rho) / e0inf : 0;
            }
            return result;
        }

        case 'kinetic_energy':
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                if (rho <= 0) continue;
                const u = solution.rhou[i] / rho;
                const v = solution.rhov[i] / rho;
                const w = solution.rhow[i] / rho;
                result[i] = 0.5 * (u * u + v * v + w * w);
            }
            return result;

        case 'normalized_kinetic_energy': {
            const ekInf = Math.max(Number.EPSILON, 0.5 * vinfSq);
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                if (rho <= 0) continue;
                const u = solution.rhou[i] / rho;
                const v = solution.rhov[i] / rho;
                const w = solution.rhow[i] / rho;
                result[i] = 0.5 * (u * u + v * v + w * w) / ekInf;
            }
            return result;
        }

        case 'velocity_magnitude':
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                if (rho <= 0) continue;
                const u = solution.rhou[i] / rho;
                const v = solution.rhov[i] / rho;
                const w = solution.rhow[i] / rho;
                result[i] = Math.sqrt(u * u + v * v + w * w);
            }
            return result;

        case 'u_velocity':
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                result[i] = rho > 0 ? solution.rhou[i] / rho : 0;
            }
            return result;

        case 'v_velocity':
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                result[i] = rho > 0 ? solution.rhov[i] / rho : 0;
            }
            return result;

        case 'w_velocity':
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                result[i] = rho > 0 ? solution.rhow[i] / rho : 0;
            }
            return result;

        case 'mach_number':
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                if (rho <= 0) continue;
                const g = solution.gamma ? solution.gamma[i] : DEFAULT_GAMMA;
                const p = pressure(i);
                const u = solution.rhou[i] / rho;
                const v = solution.rhov[i] / rho;
                const w = solution.rhow[i] / rho;
                const v2 = u * u + v * v + w * w;
                const c = Math.sqrt(Math.max(0, g * p / rho));
                result[i] = c > 0 ? Math.sqrt(v2) / c : 0;
            }
            return result;

        case 'speed_of_sound':
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                if (rho <= 0) continue;
                const g = solution.gamma ? solution.gamma[i] : DEFAULT_GAMMA;
                result[i] = Math.sqrt(Math.max(0, g * pressure(i) / rho));
            }
            return result;

        case 'cross_flow_velocity':
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                if (rho <= 0) continue;
                const v = solution.rhov[i] / rho;
                const w = solution.rhow[i] / rho;
                result[i] = Math.sqrt(v * v + w * w);
            }
            return result;

        case 'momentum_x':
            return new Float32Array(solution.rhou);

        case 'momentum_y':
            return new Float32Array(solution.rhov);

        case 'momentum_z':
            return new Float32Array(solution.rhow);

        case 'energy':
            return new Float32Array(solution.rhoe);

        case 'entropy':
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                if (rho <= 0) continue;
                const g = solution.gamma ? solution.gamma[i] : DEFAULT_GAMMA;
                const p = pressure(i);
                const arg = p * g / Math.pow(rho, g);
                result[i] = arg > 0 ? Math.log(arg) / (g - 1) : 0;
            }
            return result;

        case 'entropy_measure_s1':
            for (let i = 0; i < n; i++) {
                const rho = solution.rho[i];
                if (rho <= 0) continue;
                const g = solution.gamma ? solution.gamma[i] : DEFAULT_GAMMA;
                result[i] = pressure(i) * g / Math.pow(rho, g) - 1;
            }
            return result;

        // Derivative-based fields: return zeros (backend computes these via grid)
        case 'normalized_2d_stream_function':
        case 'velocity_divergence':
        case 'vorticity_x':
        case 'vorticity_y':
        case 'vorticity_z':
        case 'vorticity_magnitude':
        case 'swirl':
        case 'velocity_cross_vorticity_magnitude':
        case 'helicity_density':
        case 'relative_helicity':
        case 'filtered_relative_helicity':
        case 'shock_function_pressure_gradient':
        case 'filtered_shock_function':
        case 'pressure_gradient_magnitude':
        case 'density_gradient_magnitude':
            return new Float32Array(n);

        default:
            return new Float32Array(solution.rho);
    }
}

/**
 * Get statistics for a scalar field
 */
export interface FieldStats {
    min: number;
    max: number;
    mean: number;
    stdDev: number;
}

export function getFieldStats(values: Float32Array): FieldStats {
    if (values.length === 0) {
        return { min: 0, max: 0, mean: 0, stdDev: 0 };
    }

    let min = values[0];
    let max = values[0];
    let sum = 0;

    // Find min, max, and sum
    for (let i = 0; i < values.length; i++) {
        const v = values[i];
        if (v < min) min = v;
        if (v > max) max = v;
        sum += v;
    }

    const mean = sum / values.length;

    // Calculate standard deviation
    let sumSquaredDiff = 0;
    for (let i = 0; i < values.length; i++) {
        const diff = values[i] - mean;
        sumSquaredDiff += diff * diff;
    }
    const stdDev = Math.sqrt(sumSquaredDiff / values.length);

    return { min, max, mean, stdDev };
}

/**
 * Get the display name and unit for a scalar field
 */
export function getFieldInfo(field: ScalarField): ScalarFieldInfo {
    const info = SCALAR_FIELDS.find(f => f.field === field);
    return info || SCALAR_FIELDS[0];
}

/**
 * Format a numeric value for display
 */
export function formatValue(value: number, decimals: number = 3): string {
    if (!isFinite(value)) return 'N/A';

    const abs = Math.abs(value);

    if (abs === 0) {
        return '0';
    } else if (abs < 0.001) {
        return value.toExponential(decimals - 1);
    } else if (abs < 1) {
        return value.toFixed(decimals);
    } else if (abs < 1000) {
        return value.toFixed(Math.max(0, decimals - Math.floor(Math.log10(abs)) - 1));
    } else {
        return value.toExponential(decimals - 1);
    }
}
