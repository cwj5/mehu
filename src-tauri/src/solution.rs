/// Solution data visualization and computation functions
use crate::plot3d::{Plot3DGrid, Plot3DSolution};

/// Color scheme types for visualization
#[derive(Debug, Clone)]
pub enum ColorScheme {
    Viridis,
    Turbo,
    Rainbow,
    Hot,
    Grayscale,
}

impl ColorScheme {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "viridis" => Some(ColorScheme::Viridis),
            "turbo" => Some(ColorScheme::Turbo),
            "rainbow" => Some(ColorScheme::Rainbow),
            "hot" => Some(ColorScheme::Hot),
            "grayscale" => Some(ColorScheme::Grayscale),
            _ => None,
        }
    }
}

use crate::plot_state::ScalarField;

// ─────────────────────────────────────────────────────────────────────────────
// Non-dimensional reference constants
// ─────────────────────────────────────────────────────────────────────────────

/// Default ratio of specific heats for a perfect diatomic gas (air).
const DEFAULT_GAMMA: f32 = 1.4;

// ─────────────────────────────────────────────────────────────────────────────
// Resolved freestream reference state
// ─────────────────────────────────────────────────────────────────────────────

/// Resolved freestream reference conditions for a non-dimensional PLOT3D dataset.
///
/// Per the PLOT3D convention: ρ_∞ = 1, c_∞ = 1, so V_∞ = M_∞, p_∞ = 1/γ.
struct RefState {
    gamma: f32,   // ratio of specific heats (DEFAULT_GAMMA)
    p_inf: f32,   // freestream pressure = 1/γ
    minf: f32,    // freestream Mach (from metadata fsmach/refmach, else 1.0)
    vinf_sq: f32, // V_∞² = M_∞²
}

impl RefState {
    fn from_solution(solution: &Plot3DSolution) -> Self {
        let gamma = solution
            .metadata
            .as_ref()
            .and_then(|m| m.gaminf)
            .unwrap_or(DEFAULT_GAMMA);
        let minf = solution
            .metadata
            .as_ref()
            .and_then(|m| m.fsmach.or(m.refmach))
            .unwrap_or(1.0);
        RefState {
            gamma,
            p_inf: 1.0 / gamma,
            minf,
            vinf_sq: minf * minf,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Per-point primitive variables
// ─────────────────────────────────────────────────────────────────────────────

struct Pt {
    rho: f32,
    u: f32,
    v: f32,
    w: f32,
    e0: f32,   // specific total energy = rhoe/rho
    ei: f32,   // specific internal energy = e0 − ½V²
    p: f32,    // static pressure ≥ 0
    c: f32,    // speed of sound ≥ 0
    mach: f32, // local Mach number ≥ 0
    gamma: f32,
}

impl Pt {
    #[inline]
    fn at(idx: usize, solution: &Plot3DSolution) -> Option<Self> {
        let rho = solution.rho[idx];
        if rho <= 0.0 {
            return None;
        }
        let gamma = solution
            .gamma
            .as_ref()
            .map(|g| g[idx])
            .unwrap_or(DEFAULT_GAMMA);
        let u = solution.rhou[idx] / rho;
        let v = solution.rhov[idx] / rho;
        let w = solution.rhow[idx] / rho;
        let v2 = u * u + v * v + w * w;
        let e0 = solution.rhoe[idx] / rho;
        let ei = e0 - 0.5 * v2;
        let p = ((gamma - 1.0) * rho * ei).max(0.0);
        let c = (gamma * p / rho).max(0.0).sqrt();
        let mach = if c > 0.0 { v2.sqrt() / c } else { 0.0 };
        Some(Pt {
            rho,
            u,
            v,
            w,
            e0,
            ei,
            p,
            c,
            mach,
            gamma,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Pitot pressure helper
// ─────────────────────────────────────────────────────────────────────────────

/// Pitot (stagnation-behind-shock) pressure.
///   M < 1 → isentropic p₀ = p·[1+(γ-1)/2·M²]^(γ/(γ-1))
///   M ≥ 1 → Rayleigh pitot formula (normal shock + isentropic compression)
fn pitot_pressure(p: f32, mach: f32, gamma: f32) -> f32 {
    let m2 = mach * mach;
    let gm1 = gamma - 1.0;
    let gp1 = gamma + 1.0;
    if mach < 1.0 {
        let base = 1.0 + gm1 / 2.0 * m2;
        p * base.powf(gamma / gm1)
    } else {
        let numer = (gp1 / 2.0 * m2).powf(gamma / gm1);
        let denom_base = (2.0 * gamma * m2 - gm1) / gp1;
        let denom = if denom_base > 0.0 {
            denom_base.powf(1.0 / gm1)
        } else {
            f32::EPSILON
        };
        p * numer / denom
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// compute_scalar_field
// ─────────────────────────────────────────────────────────────────────────────

/// Compute a scalar field from solution data.
///
/// All 48 scalar-100-series functions are handled here except those that require
/// spatial derivatives (vorticity, divergence, gradients, shock functions,
/// helicity, and 2D stream function).  Derivative fields return an all-zeros
/// array; use `compute_scalar_field_with_grid` for those.
#[allow(dead_code)]
pub fn compute_scalar_field(solution: &Plot3DSolution, field: ScalarField) -> Vec<f32> {
    let n = solution.rho.len();
    let ref_state = RefState::from_solution(solution);

    match field {
        ScalarField::None => vec![0.0; n],

        // ── Q-file raw quantities ─────────────────────────────────────────────
        ScalarField::Density => solution.rho.clone(),
        ScalarField::MomentumX => solution.rhou.clone(),
        ScalarField::MomentumY => solution.rhov.clone(),
        ScalarField::MomentumZ => solution.rhow.clone(),
        ScalarField::Energy => solution.rhoe.clone(),

        ScalarField::UVelocity => (0..n)
            .map(|i| {
                let rho = solution.rho[i];
                if rho > 0.0 {
                    solution.rhou[i] / rho
                } else {
                    0.0
                }
            })
            .collect(),

        ScalarField::VVelocity => (0..n)
            .map(|i| {
                let rho = solution.rho[i];
                if rho > 0.0 {
                    solution.rhov[i] / rho
                } else {
                    0.0
                }
            })
            .collect(),

        ScalarField::WVelocity => (0..n)
            .map(|i| {
                let rho = solution.rho[i];
                if rho > 0.0 {
                    solution.rhow[i] / rho
                } else {
                    0.0
                }
            })
            .collect(),

        ScalarField::VelocityMagnitude => (0..n)
            .map(|i| {
                let rho = solution.rho[i];
                if rho > 0.0 {
                    let u = solution.rhou[i] / rho;
                    let v = solution.rhov[i] / rho;
                    let w = solution.rhow[i] / rho;
                    (u * u + v * v + w * w).sqrt()
                } else {
                    0.0
                }
            })
            .collect(),

        ScalarField::Pressure => (0..n)
            .map(|i| {
                let rho = solution.rho[i];
                if rho > 0.0 {
                    let gamma = solution
                        .gamma
                        .as_ref()
                        .map(|g| g[i])
                        .unwrap_or(DEFAULT_GAMMA);
                    let u = solution.rhou[i] / rho;
                    let v = solution.rhov[i] / rho;
                    let w = solution.rhow[i] / rho;
                    let ek = 0.5 * rho * (u * u + v * v + w * w);
                    ((gamma - 1.0) * (solution.rhoe[i] - ek)).max(0.0)
                } else {
                    0.0
                }
            })
            .collect(),

        // ── Density family (101-104) ──────────────────────────────────────────

        // 101: ρ/ρ_∞ = ρ  (ρ_∞ = 1 in non-dimensional PLOT3D)
        ScalarField::NormalizedDensity => solution.rho.clone(),

        // 102: ρ₀ = ρ·[1+(γ-1)/2·M²]^(1/(γ-1))
        ScalarField::StagnationDensity => (0..n)
            .map(|i| match Pt::at(i, solution) {
                None => 0.0,
                Some(pt) => {
                    let base = 1.0 + (pt.gamma - 1.0) / 2.0 * pt.mach * pt.mach;
                    pt.rho * base.powf(1.0 / (pt.gamma - 1.0))
                }
            })
            .collect(),

        // 103: ρ₀/ρ_∞ = ρ₀  (ρ_∞ = 1)
        ScalarField::NormalizedStagnationDensity => (0..n)
            .map(|i| match Pt::at(i, solution) {
                None => 0.0,
                Some(pt) => {
                    let base = 1.0 + (pt.gamma - 1.0) / 2.0 * pt.mach * pt.mach;
                    pt.rho * base.powf(1.0 / (pt.gamma - 1.0))
                }
            })
            .collect(),

        // 104: ln(ρ/ρ_∞) = ln(ρ)
        ScalarField::LogNormalizedDensity => (0..n)
            .map(|i| {
                let rho = solution.rho[i];
                if rho > 0.0 {
                    rho.ln()
                } else {
                    0.0
                }
            })
            .collect(),

        // ── Pressure family (111-119) ─────────────────────────────────────────

        // 111: p/p_∞ = p·γ∞  (p_∞ = 1/γ∞)
        ScalarField::NormalizedPressure => {
            let g_inf = ref_state.gamma;
            (0..n)
                .map(|i| match Pt::at(i, solution) {
                    None => 0.0,
                    Some(pt) => pt.p * g_inf,
                })
                .collect()
        }

        // 112: p₀ = p·[1+(γ-1)/2·M²]^(γ/(γ-1))
        ScalarField::StagnationPressure => (0..n)
            .map(|i| match Pt::at(i, solution) {
                None => 0.0,
                Some(pt) => {
                    let base = 1.0 + (pt.gamma - 1.0) / 2.0 * pt.mach * pt.mach;
                    pt.p * base.powf(pt.gamma / (pt.gamma - 1.0))
                }
            })
            .collect(),

        // 113: p₀/p_∞ = p₀·γ∞
        ScalarField::NormalizedStagnationPressure => {
            let g_inf = ref_state.gamma;
            (0..n)
                .map(|i| match Pt::at(i, solution) {
                    None => 0.0,
                    Some(pt) => {
                        let base = 1.0 + (pt.gamma - 1.0) / 2.0 * pt.mach * pt.mach;
                        pt.p * base.powf(pt.gamma / (pt.gamma - 1.0)) * g_inf
                    }
                })
                .collect()
        }

        // 114: Cp = (p − p_∞) / (½ρ_∞V_∞²)
        ScalarField::PressureCoefficient => {
            let p_inf = ref_state.p_inf;
            let dyn_inf = if ref_state.vinf_sq > 0.0 {
                0.5 * ref_state.vinf_sq
            } else {
                1.0
            };
            (0..n)
                .map(|i| match Pt::at(i, solution) {
                    None => 0.0,
                    Some(pt) => (pt.p - p_inf) / dyn_inf,
                })
                .collect()
        }

        // 115: Cp₀ = (p₀ − p₀_∞) / (½ρ_∞V_∞²)
        ScalarField::StagnationPressureCoefficient => {
            let g = ref_state.gamma;
            let minf = ref_state.minf;
            let p_inf = ref_state.p_inf;
            let p0_inf_base = 1.0 + (g - 1.0) / 2.0 * minf * minf;
            let p0_inf = p_inf * p0_inf_base.powf(g / (g - 1.0));
            let dyn_inf = if ref_state.vinf_sq > 0.0 {
                0.5 * ref_state.vinf_sq
            } else {
                1.0
            };
            (0..n)
                .map(|i| match Pt::at(i, solution) {
                    None => 0.0,
                    Some(pt) => {
                        let base = 1.0 + (pt.gamma - 1.0) / 2.0 * pt.mach * pt.mach;
                        let p0 = pt.p * base.powf(pt.gamma / (pt.gamma - 1.0));
                        (p0 - p0_inf) / dyn_inf
                    }
                })
                .collect()
        }

        // 116: pitot pressure (Rayleigh formula for M≥1, isentropic p₀ for M<1)
        ScalarField::PitotPressure => (0..n)
            .map(|i| match Pt::at(i, solution) {
                None => 0.0,
                Some(pt) => pitot_pressure(pt.p, pt.mach, pt.gamma),
            })
            .collect(),

        // 117: pp/p_∞ = pp·γ∞
        ScalarField::PitotPressureRatio => {
            let g_inf = ref_state.gamma;
            (0..n)
                .map(|i| match Pt::at(i, solution) {
                    None => 0.0,
                    Some(pt) => pitot_pressure(pt.p, pt.mach, pt.gamma) * g_inf,
                })
                .collect()
        }

        // 118: q = ½ρV²
        ScalarField::DynamicPressure => (0..n)
            .map(|i| {
                let rho = solution.rho[i];
                if rho > 0.0 {
                    let u = solution.rhou[i] / rho;
                    let v = solution.rhov[i] / rho;
                    let w = solution.rhow[i] / rho;
                    0.5 * rho * (u * u + v * v + w * w)
                } else {
                    0.0
                }
            })
            .collect(),

        // 119: ln(p/p_∞) = ln(p·γ∞)
        ScalarField::LogNormalizedPressure => {
            let g_inf = ref_state.gamma;
            (0..n)
                .map(|i| match Pt::at(i, solution) {
                    None => 0.0,
                    Some(pt) => {
                        let norm_p = pt.p * g_inf;
                        if norm_p > 0.0 {
                            norm_p.ln()
                        } else {
                            0.0
                        }
                    }
                })
                .collect()
        }

        // ── Temperature family (120-124) ──────────────────────────────────────

        // 120: T = p/(ρR) = p/ρ  (R = 1 non-dimensional)
        ScalarField::Temperature => (0..n)
            .map(|i| match Pt::at(i, solution) {
                None => 0.0,
                Some(pt) => pt.p / pt.rho,
            })
            .collect(),

        // 121: T/T_∞ = T·γ∞  (T_∞ = p_∞/(ρ_∞R) = 1/γ∞)
        ScalarField::NormalizedTemperature => {
            let g_inf = ref_state.gamma;
            (0..n)
                .map(|i| match Pt::at(i, solution) {
                    None => 0.0,
                    Some(pt) => pt.p / pt.rho * g_inf,
                })
                .collect()
        }

        // 122: T₀ = T·[1+(γ-1)/2·M²]
        ScalarField::StagnationTemperature => (0..n)
            .map(|i| match Pt::at(i, solution) {
                None => 0.0,
                Some(pt) => {
                    let base = 1.0 + (pt.gamma - 1.0) / 2.0 * pt.mach * pt.mach;
                    (pt.p / pt.rho) * base
                }
            })
            .collect(),

        // 123: T₀/T_∞ = T₀·γ∞
        ScalarField::NormalizedStagnationTemperature => {
            let g_inf = ref_state.gamma;
            (0..n)
                .map(|i| match Pt::at(i, solution) {
                    None => 0.0,
                    Some(pt) => {
                        let base = 1.0 + (pt.gamma - 1.0) / 2.0 * pt.mach * pt.mach;
                        (pt.p / pt.rho) * base * g_inf
                    }
                })
                .collect()
        }

        // 124: ln(T/T_∞) = ln(T·γ∞)
        ScalarField::LogNormalizedTemperature => {
            let g_inf = ref_state.gamma;
            (0..n)
                .map(|i| match Pt::at(i, solution) {
                    None => 0.0,
                    Some(pt) => {
                        let norm_t = pt.p / pt.rho * g_inf;
                        if norm_t > 0.0 {
                            norm_t.ln()
                        } else {
                            0.0
                        }
                    }
                })
                .collect()
        }

        // ── Enthalpy family (130-133) ─────────────────────────────────────────

        // 130: h = γ·eᵢ  (static enthalpy per unit mass)
        ScalarField::Enthalpy => (0..n)
            .map(|i| match Pt::at(i, solution) {
                None => 0.0,
                Some(pt) => pt.gamma * pt.ei,
            })
            .collect(),

        // 131: h/h_∞ = h·(γ∞-1)  (h_∞ = 1/(γ∞-1))
        ScalarField::NormalizedEnthalpy => {
            let g_inf = ref_state.gamma;
            (0..n)
                .map(|i| match Pt::at(i, solution) {
                    None => 0.0,
                    Some(pt) => pt.gamma * pt.ei * (g_inf - 1.0),
                })
                .collect()
        }

        // 132: h₀ = e₀ + p/ρ  (total enthalpy per unit mass)
        ScalarField::StagnationEnthalpy => (0..n)
            .map(|i| match Pt::at(i, solution) {
                None => 0.0,
                Some(pt) => pt.e0 + pt.p / pt.rho,
            })
            .collect(),

        // 133: h₀/h₀_∞  (h₀_∞ = 1/(γ-1) + ½V_∞²)
        ScalarField::NormalizedStagnationEnthalpy => {
            let g = ref_state.gamma;
            let h0_inf = (1.0 / (g - 1.0) + 0.5 * ref_state.vinf_sq).max(f32::EPSILON);
            (0..n)
                .map(|i| match Pt::at(i, solution) {
                    None => 0.0,
                    Some(pt) => (pt.e0 + pt.p / pt.rho) / h0_inf,
                })
                .collect()
        }

        // ── Energy family (140-145) ───────────────────────────────────────────

        // 140: eᵢ = e₀ − ½V²  (specific internal energy)
        ScalarField::InternalEnergy => (0..n)
            .map(|i| match Pt::at(i, solution) {
                None => 0.0,
                Some(pt) => pt.ei,
            })
            .collect(),

        // 141: eᵢ/eᵢ_∞ = eᵢ·γ∞(γ∞-1)  (eᵢ_∞ = 1/(γ∞(γ∞-1)))
        ScalarField::NormalizedInternalEnergy => {
            let g_inf = ref_state.gamma;
            (0..n)
                .map(|i| match Pt::at(i, solution) {
                    None => 0.0,
                    Some(pt) => pt.ei * g_inf * (g_inf - 1.0),
                })
                .collect()
        }

        // 142: e₀ = Q5/ρ  (specific stagnation energy per unit mass)
        ScalarField::StagnationEnergy => (0..n)
            .map(|i| {
                let rho = solution.rho[i];
                if rho > 0.0 {
                    solution.rhoe[i] / rho
                } else {
                    0.0
                }
            })
            .collect(),

        // 143: e₀/e₀_∞  (e₀_∞ = eᵢ_∞ + ½V_∞²)
        ScalarField::NormalizedStagnationEnergy => {
            let g = ref_state.gamma;
            let ei_inf = 1.0 / (g * (g - 1.0));
            let e0_inf = (ei_inf + 0.5 * ref_state.vinf_sq).max(f32::EPSILON);
            (0..n)
                .map(|i| {
                    let rho = solution.rho[i];
                    if rho > 0.0 {
                        (solution.rhoe[i] / rho) / e0_inf
                    } else {
                        0.0
                    }
                })
                .collect()
        }

        // 144: eₖ = ½V²  (specific kinetic energy per unit mass)
        ScalarField::KineticEnergy => (0..n)
            .map(|i| {
                let rho = solution.rho[i];
                if rho > 0.0 {
                    let u = solution.rhou[i] / rho;
                    let v = solution.rhov[i] / rho;
                    let w = solution.rhow[i] / rho;
                    0.5 * (u * u + v * v + w * w)
                } else {
                    0.0
                }
            })
            .collect(),

        // 145: eₖ/eₖ_∞  (eₖ_∞ = ½V_∞²)
        ScalarField::NormalizedKineticEnergy => {
            let ek_inf = (0.5 * ref_state.vinf_sq).max(f32::EPSILON);
            (0..n)
                .map(|i| {
                    let rho = solution.rho[i];
                    if rho > 0.0 {
                        let u = solution.rhou[i] / rho;
                        let v = solution.rhov[i] / rho;
                        let w = solution.rhow[i] / rho;
                        0.5 * (u * u + v * v + w * w) / ek_inf
                    } else {
                        0.0
                    }
                })
                .collect()
        }

        // ── Velocity / flow family (154-156) ──────────────────────────────────

        // 154: M = |V|/c
        ScalarField::MachNumber => (0..n)
            .map(|i| match Pt::at(i, solution) {
                None => 0.0,
                Some(pt) => pt.mach,
            })
            .collect(),

        // 155: c = √(γp/ρ)
        ScalarField::SpeedOfSound => (0..n)
            .map(|i| match Pt::at(i, solution) {
                None => 0.0,
                Some(pt) => pt.c,
            })
            .collect(),

        // 156: V_cf = √(v² + w²)  (cross-flow speed)
        ScalarField::CrossFlowVelocity => (0..n)
            .map(|i| {
                let rho = solution.rho[i];
                if rho > 0.0 {
                    let v = solution.rhov[i] / rho;
                    let w = solution.rhow[i] / rho;
                    (v * v + w * w).sqrt()
                } else {
                    0.0
                }
            })
            .collect(),

        // ── Entropy family (170-171) ──────────────────────────────────────────

        // 170: s = cᵥ·ln[(p/p_∞)/(ρ/ρ_∞)^γ]  = [1/(γ-1)]·ln(p·γ / ρ^γ)
        ScalarField::Entropy => (0..n)
            .map(|i| match Pt::at(i, solution) {
                None => 0.0,
                Some(pt) => {
                    if pt.rho > 0.0 {
                        let arg = pt.p * pt.gamma / pt.rho.powf(pt.gamma);
                        if arg > 0.0 {
                            arg.ln() / (pt.gamma - 1.0)
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    }
                }
            })
            .collect(),

        // 171: s₁ = (p/p_∞)/(ρ/ρ_∞)^γ − 1 = p·γ/ρ^γ − 1
        ScalarField::EntropyMeasureS1 => (0..n)
            .map(|i| match Pt::at(i, solution) {
                None => 0.0,
                Some(pt) => {
                    if pt.rho > 0.0 {
                        pt.p * pt.gamma / pt.rho.powf(pt.gamma) - 1.0
                    } else {
                        0.0
                    }
                }
            })
            .collect(),

        // ── Derivative-based fields (require grid coordinates) ────────────────
        // Return zeros; use compute_scalar_field_with_grid for these.
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
        | ScalarField::DensityGradientMagnitude => vec![0.0; n],
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// compute_scalar_field_surface
// ─────────────────────────────────────────────────────────────────────────────

/// Compute scalar field for the k=0 surface with optional decimation, using
/// grid coordinates for derivative-based fields (vorticity, divergence, gradients, etc.).
pub fn compute_scalar_field_surface_with_grid(
    solution: &Plot3DSolution,
    grid: &Plot3DGrid,
    field: ScalarField,
    decimation_factor: usize,
) -> Vec<f32> {
    let decimation = decimation_factor.max(1);
    let ni = solution.dimensions.i as usize;
    let nj = solution.dimensions.j as usize;
    let i_decimated = ((ni - 1) / decimation) + 1;
    let j_decimated = ((nj - 1) / decimation) + 1;

    let full_field = compute_scalar_field_with_grid(solution, grid, field);

    let mut values = Vec::with_capacity(i_decimated * j_decimated);
    for j_step in 0..j_decimated {
        let j_idx = (j_step * decimation).min(nj - 1);
        for i_step in 0..i_decimated {
            let i_idx = (i_step * decimation).min(ni - 1);
            let idx = j_idx * ni + i_idx; // k=0 surface
            values.push(full_field[idx]);
        }
    }
    values
}

// ─────────────────────────────────────────────────────────────────────────────
// Curvilinear-metric gradient helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Finite differences of `data` in all three computational directions at
/// point (gi, gj, gk) on a ni×nj×nk structured grid.
///
/// Uses central differences at interior points and one-sided at boundaries.
/// Returns raw differences (not divided by ΔΞ = 1), which is consistent with
/// the grid metric differences used in `physical_grad_at`.
#[inline(always)]
fn cd_at(
    data: &[f32],
    ni: usize,
    nj: usize,
    nk: usize,
    gi: usize,
    gj: usize,
    gk: usize,
) -> (f32, f32, f32) {
    let idx = |i: usize, j: usize, k: usize| i + j * ni + k * ni * nj;

    let f_xi = if ni < 2 {
        0.0
    } else if gi == 0 {
        data[idx(1, gj, gk)] - data[idx(0, gj, gk)]
    } else if gi == ni - 1 {
        data[idx(ni - 1, gj, gk)] - data[idx(ni - 2, gj, gk)]
    } else {
        data[idx(gi + 1, gj, gk)] - data[idx(gi - 1, gj, gk)]
    };

    let f_eta = if nj < 2 {
        0.0
    } else if gj == 0 {
        data[idx(gi, 1, gk)] - data[idx(gi, 0, gk)]
    } else if gj == nj - 1 {
        data[idx(gi, nj - 1, gk)] - data[idx(gi, nj - 2, gk)]
    } else {
        data[idx(gi, gj + 1, gk)] - data[idx(gi, gj - 1, gk)]
    };

    let f_zeta = if nk < 2 {
        0.0
    } else if gk == 0 {
        data[idx(gi, gj, 1)] - data[idx(gi, gj, 0)]
    } else if gk == nk - 1 {
        data[idx(gi, gj, nk - 1)] - data[idx(gi, gj, nk - 2)]
    } else {
        data[idx(gi, gj, gk + 1)] - data[idx(gi, gj, gk - 1)]
    };

    (f_xi, f_eta, f_zeta)
}

/// Physical gradient of `data` at (gi, gj, gk) via curvilinear metric inversion.
///
/// Computes (∂f/∂x, ∂f/∂y, ∂f/∂z) using the 3×3 Jacobian inverse.
/// Returns (0,0,0) for degenerate cells (zero-volume or non-manifold).
fn physical_grad_at(
    data: &[f32],
    ni: usize,
    nj: usize,
    nk: usize,
    gi: usize,
    gj: usize,
    gk: usize,
    x: &[f32],
    y: &[f32],
    z: &[f32],
) -> (f32, f32, f32) {
    let (f_xi, f_eta, f_zeta) = cd_at(data, ni, nj, nk, gi, gj, gk);
    let (x_xi, x_eta, x_zeta) = cd_at(x, ni, nj, nk, gi, gj, gk);
    let (y_xi, y_eta, y_zeta) = cd_at(y, ni, nj, nk, gi, gj, gk);
    let (z_xi, z_eta, z_zeta) = cd_at(z, ni, nj, nk, gi, gj, gk);

    // 2-D fallback: when the grid is planar (nk==1 or all ζ-direction coordinate
    // differences are zero), the 3×3 Jacobian is singular.  Use the 2×2
    // in-plane Jacobian instead; the out-of-plane gradient component is 0.
    let planar = nk <= 1
        || (x_zeta.abs() < 1e-20
            && y_zeta.abs() < 1e-20
            && z_zeta.abs() < 1e-20
            && z_xi.abs() < 1e-20
            && z_eta.abs() < 1e-20);
    if planar {
        // J_2d = [[x_xi, x_eta], [y_xi, y_eta]]
        let det2 = x_xi * y_eta - x_eta * y_xi;
        if det2.abs() < 1e-20 {
            return (0.0, 0.0, 0.0);
        }
        let grad_x = (f_xi * y_eta - f_eta * y_xi) / det2;
        let grad_y = (f_eta * x_xi - f_xi * x_eta) / det2;
        return (grad_x, grad_y, 0.0);
    }

    // Jacobian J = [[a,b,c],[d,e,f],[g,h,s]]  (rows x,y,z; cols ξ,η,ζ)
    let (a, b, c) = (x_xi, x_eta, x_zeta);
    let (d, e, f) = (y_xi, y_eta, y_zeta);
    let (g, h, s) = (z_xi, z_eta, z_zeta);

    let det = a * (e * s - f * h) - b * (d * s - f * g) + c * (d * h - e * g);
    if det.abs() < 1e-20 {
        return (0.0, 0.0, 0.0);
    }

    let grad_x =
        (f_xi * (e * s - f * h) + f_eta * (f * g - d * s) + f_zeta * (d * h - e * g)) / det;
    let grad_y =
        (f_xi * (c * h - b * s) + f_eta * (a * s - c * g) + f_zeta * (b * g - a * h)) / det;
    let grad_z =
        (f_xi * (b * f - c * e) + f_eta * (c * d - a * f) + f_zeta * (a * e - b * d)) / det;

    (grad_x, grad_y, grad_z)
}

// ─────────────────────────────────────────────────────────────────────────────
// compute_scalar_field_with_grid
// ─────────────────────────────────────────────────────────────────────────────

/// Compute any scalar field, including derivative-based fields that require
/// grid coordinates (vorticity, divergence, gradients, helicity, shock
/// functions, 2D stream function).
///
/// For non-derivative fields, delegates to `compute_scalar_field`.
#[allow(dead_code)]
pub fn compute_scalar_field_with_grid(
    solution: &Plot3DSolution,
    grid: &Plot3DGrid,
    field: ScalarField,
) -> Vec<f32> {
    match field {
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
        | ScalarField::DensityGradientMagnitude => compute_derivative_field(solution, grid, field),
        _ => compute_scalar_field(solution, field),
    }
}

fn compute_derivative_field(
    solution: &Plot3DSolution,
    grid: &Plot3DGrid,
    field: ScalarField,
) -> Vec<f32> {
    let ni = solution.dimensions.i as usize;
    let nj = solution.dimensions.j as usize;
    let nk = solution.dimensions.k as usize;
    let n = solution.rho.len();
    let ref_state = RefState::from_solution(solution);

    let x = &grid.x_coords;
    let y = &grid.y_coords;
    let z = &grid.z_coords;

    // Precompute velocity components once
    let mut u_f = vec![0.0f32; n];
    let mut v_f = vec![0.0f32; n];
    let mut w_f = vec![0.0f32; n];
    for i in 0..n {
        let rho = solution.rho[i];
        if rho > 0.0 {
            u_f[i] = solution.rhou[i] / rho;
            v_f[i] = solution.rhov[i] / rho;
            w_f[i] = solution.rhow[i] / rho;
        }
    }

    match field {
        // 157: normalized 2D stream function
        // ψ(i,j,k) = Σ_{j'<j} ρu(i,j',k)·Δy(i,j',j'+1,k) / M_∞
        ScalarField::Normalized2dStreamFunction => {
            let norm = ref_state.minf.max(f32::EPSILON);
            let mut result = vec![0.0f32; n];
            for gk in 0..nk {
                for gi in 0..ni {
                    let mut psi = 0.0f32;
                    for gj in 1..nj {
                        let prev = gi + (gj - 1) * ni + gk * ni * nj;
                        let curr = gi + gj * ni + gk * ni * nj;
                        let dy = y[curr] - y[prev];
                        psi += solution.rhou[prev] * dy;
                        result[curr] = psi / norm;
                    }
                }
            }
            result
        }

        // 158: div(V) = −(1/ρ)·V·∇ρ  (steady-flow continuity, DIVV subroutine)
        ScalarField::VelocityDivergence => {
            let mut result = vec![0.0f32; n];
            for gk in 0..nk {
                for gj in 0..nj {
                    for gi in 0..ni {
                        let idx = gi + gj * ni + gk * ni * nj;
                        let rho = solution.rho[idx];
                        if rho > 0.0 {
                            let (drx, dry, drz) =
                                physical_grad_at(&solution.rho, ni, nj, nk, gi, gj, gk, x, y, z);
                            result[idx] = -(u_f[idx] * drx + v_f[idx] * dry + w_f[idx] * drz) / rho;
                        }
                    }
                }
            }
            result
        }

        // 180-188: vorticity-derived fields (compute ω once, dispatch on variant)
        ScalarField::VorticityX
        | ScalarField::VorticityY
        | ScalarField::VorticityZ
        | ScalarField::VorticityMagnitude
        | ScalarField::Swirl
        | ScalarField::VelocityCrossVorticityMagnitude
        | ScalarField::HelicityDensity
        | ScalarField::RelativeHelicity
        | ScalarField::FilteredRelativeHelicity => {
            let mut ox = vec![0.0f32; n];
            let mut oy = vec![0.0f32; n];
            let mut oz = vec![0.0f32; n];

            for gk in 0..nk {
                for gj in 0..nj {
                    for gi in 0..ni {
                        let idx = gi + gj * ni + gk * ni * nj;
                        let (_dux, duy, duz) =
                            physical_grad_at(&u_f, ni, nj, nk, gi, gj, gk, x, y, z);
                        let (dvx, _dvy, dvz) =
                            physical_grad_at(&v_f, ni, nj, nk, gi, gj, gk, x, y, z);
                        let (dwx, dwy, _dwz) =
                            physical_grad_at(&w_f, ni, nj, nk, gi, gj, gk, x, y, z);
                        ox[idx] = dwy - dvz; // ω₁ = ∂w/∂y − ∂v/∂z
                        oy[idx] = duz - dwx; // ω₂ = ∂u/∂z − ∂w/∂x
                        oz[idx] = dvx - duy; // ω₃ = ∂v/∂x − ∂u/∂y
                    }
                }
            }

            match field {
                ScalarField::VorticityX => ox,
                ScalarField::VorticityY => oy,
                ScalarField::VorticityZ => oz,

                ScalarField::VorticityMagnitude => (0..n)
                    .map(|i| (ox[i] * ox[i] + oy[i] * oy[i] + oz[i] * oz[i]).sqrt())
                    .collect(),

                ScalarField::Swirl => (0..n)
                    .map(|i| {
                        let v2 = u_f[i] * u_f[i] + v_f[i] * v_f[i] + w_f[i] * w_f[i];
                        let rho = solution.rho[i];
                        if v2 > 0.0 && rho > 0.0 {
                            (ox[i] * u_f[i] + oy[i] * v_f[i] + oz[i] * w_f[i]) / (rho * v2)
                        } else {
                            0.0
                        }
                    })
                    .collect(),

                ScalarField::VelocityCrossVorticityMagnitude => (0..n)
                    .map(|i| {
                        let cx = v_f[i] * oz[i] - w_f[i] * oy[i];
                        let cy = w_f[i] * ox[i] - u_f[i] * oz[i];
                        let cz = u_f[i] * oy[i] - v_f[i] * ox[i];
                        (cx * cx + cy * cy + cz * cz).sqrt()
                    })
                    .collect(),

                ScalarField::HelicityDensity => (0..n)
                    .map(|i| u_f[i] * ox[i] + v_f[i] * oy[i] + w_f[i] * oz[i])
                    .collect(),

                ScalarField::RelativeHelicity => (0..n)
                    .map(|i| {
                        let v_sq = u_f[i] * u_f[i] + v_f[i] * v_f[i] + w_f[i] * w_f[i];
                        let o_sq = ox[i] * ox[i] + oy[i] * oy[i] + oz[i] * oz[i];
                        if v_sq > 0.0 && o_sq > 0.0 {
                            let dot = u_f[i] * ox[i] + v_f[i] * oy[i] + w_f[i] * oz[i];
                            dot / (v_sq.sqrt() * o_sq.sqrt())
                        } else {
                            0.0
                        }
                    })
                    .collect(),

                ScalarField::FilteredRelativeHelicity => {
                    let threshold = 0.1 * ref_state.vinf_sq;
                    (0..n)
                        .map(|i| {
                            let v_sq = u_f[i] * u_f[i] + v_f[i] * v_f[i] + w_f[i] * w_f[i];
                            let o_sq = ox[i] * ox[i] + oy[i] * oy[i] + oz[i] * oz[i];
                            if v_sq > 0.0 && o_sq > 0.0 {
                                let dot = u_f[i] * ox[i] + v_f[i] * oy[i] + w_f[i] * oz[i];
                                if dot.abs() >= threshold {
                                    dot / (v_sq.sqrt() * o_sq.sqrt())
                                } else {
                                    0.0
                                }
                            } else {
                                0.0
                            }
                        })
                        .collect()
                }

                _ => unreachable!(),
            }
        }

        // 190-192: pressure-gradient / shock fields
        ScalarField::ShockFunctionPressureGradient
        | ScalarField::FilteredShockFunction
        | ScalarField::PressureGradientMagnitude => {
            let pressure = compute_scalar_field(solution, ScalarField::Pressure);
            let mut gpx = vec![0.0f32; n];
            let mut gpy = vec![0.0f32; n];
            let mut gpz = vec![0.0f32; n];

            for gk in 0..nk {
                for gj in 0..nj {
                    for gi in 0..ni {
                        let idx = gi + gj * ni + gk * ni * nj;
                        let (dpx, dpy, dpz) =
                            physical_grad_at(&pressure, ni, nj, nk, gi, gj, gk, x, y, z);
                        gpx[idx] = dpx;
                        gpy[idx] = dpy;
                        gpz[idx] = dpz;
                    }
                }
            }

            match field {
                ScalarField::PressureGradientMagnitude => (0..n)
                    .map(|i| (gpx[i] * gpx[i] + gpy[i] * gpy[i] + gpz[i] * gpz[i]).sqrt())
                    .collect(),

                ScalarField::ShockFunctionPressureGradient => (0..n)
                    .map(|i| {
                        let p = pressure[i];
                        if p > 0.0 {
                            let gm = (gpx[i] * gpx[i] + gpy[i] * gpy[i] + gpz[i] * gpz[i]).sqrt();
                            gm / p
                        } else {
                            0.0
                        }
                    })
                    .collect(),

                ScalarField::FilteredShockFunction => (0..n)
                    .map(|i| {
                        let p = pressure[i];
                        if p <= 0.0 {
                            return 0.0;
                        }

                        match Pt::at(i, solution) {
                            Some(pt) if pt.c > 0.0 => {
                                let vmag = (pt.u * pt.u + pt.v * pt.v + pt.w * pt.w).sqrt();
                                let mach = vmag / pt.c;
                                if mach > 1.0 {
                                    let gm = (gpx[i] * gpx[i] + gpy[i] * gpy[i] + gpz[i] * gpz[i])
                                        .sqrt();
                                    gm / p
                                } else {
                                    0.0
                                }
                            }
                            _ => 0.0,
                        }
                    })
                    .collect(),

                _ => unreachable!(),
            }
        }

        // 193: |∇ρ|
        ScalarField::DensityGradientMagnitude => {
            let mut result = vec![0.0f32; n];
            for gk in 0..nk {
                for gj in 0..nj {
                    for gi in 0..ni {
                        let idx = gi + gj * ni + gk * ni * nj;
                        let (drx, dry, drz) =
                            physical_grad_at(&solution.rho, ni, nj, nk, gi, gj, gk, x, y, z);
                        result[idx] = (drx * drx + dry * dry + drz * drz).sqrt();
                    }
                }
            }
            result
        }

        _ => unreachable!("compute_derivative_field called with non-derivative field"),
    }
}

/// Color mapping function from normalized value [0, 1] to RGB
pub fn map_value_to_color(value: f32, scheme: &ColorScheme) -> (f32, f32, f32) {
    if !value.is_finite() {
        return (0.0, 0.0, 0.0);
    }
    let v = value.max(0.0).min(1.0);
    match scheme {
        ColorScheme::Viridis => viridis_color(v),
        ColorScheme::Turbo => turbo_color(v),
        ColorScheme::Rainbow => rainbow_color(v),
        ColorScheme::Hot => hot_color(v),
        ColorScheme::Grayscale => (v, v, v),
    }
}

fn viridis_color(v: f32) -> (f32, f32, f32) {
    let lut = [
        (0.267004, 0.004874, 0.329415),
        (0.282623, 0.140461, 0.469470),
        (0.253935, 0.265254, 0.529983),
        (0.206756, 0.371758, 0.553806),
        (0.163625, 0.471133, 0.558695),
        (0.127568, 0.566949, 0.550413),
        (0.134692, 0.658636, 0.517649),
        (0.266941, 0.748751, 0.440573),
        (0.477504, 0.821444, 0.318195),
        (0.741388, 0.873449, 0.149561),
        (0.993248, 0.906157, 0.143936),
    ];
    let idx = (v * (lut.len() - 1) as f32).floor() as usize;
    let t = (v * (lut.len() - 1) as f32) - idx as f32;
    let next_idx = (idx + 1).min(lut.len() - 1);
    let (r1, g1, b1) = lut[idx];
    let (r2, g2, b2) = lut[next_idx];
    (
        r1 * (1.0 - t) + r2 * t,
        g1 * (1.0 - t) + g2 * t,
        b1 * (1.0 - t) + b2 * t,
    )
}

fn turbo_color(v: f32) -> (f32, f32, f32) {
    // Google Turbo colormap sampled at 16 key points
    let lut = [
        (0.19, 0.07, 0.23), // dark purple/blue
        (0.21, 0.14, 0.42), // purple-blue
        (0.24, 0.26, 0.61), // blue
        (0.27, 0.38, 0.81), // cyan-blue
        (0.29, 0.50, 0.93), // cyan
        (0.28, 0.63, 0.94), // cyan-green
        (0.25, 0.74, 0.80), // green
        (0.42, 0.84, 0.54), // yellow-green
        (0.67, 0.90, 0.28), // yellow
        (0.89, 0.88, 0.12), // orange-yellow
        (1.00, 0.77, 0.06), // orange
        (1.00, 0.60, 0.03), // orange-red
        (0.97, 0.40, 0.02), // red-orange
        (0.92, 0.20, 0.01), // red
        (0.85, 0.09, 0.01), // dark red
        (0.80, 0.02, 0.00), // dark red
    ];
    let idx = (v * (lut.len() - 1) as f32).floor() as usize;
    let t = (v * (lut.len() - 1) as f32) - idx as f32;
    let next_idx = (idx + 1).min(lut.len() - 1);
    let (r1, g1, b1) = lut[idx];
    let (r2, g2, b2) = lut[next_idx];
    (
        (r1 * (1.0 - t) + r2 * t).max(0.0).min(1.0),
        (g1 * (1.0 - t) + g2 * t).max(0.0).min(1.0),
        (b1 * (1.0 - t) + b2 * t).max(0.0).min(1.0),
    )
}

fn rainbow_color(v: f32) -> (f32, f32, f32) {
    let (mut r, mut g, mut b) = (0.0, 0.0, 0.0);
    if v < 0.2 {
        r = 1.0;
        g = v / 0.2;
    } else if v < 0.4 {
        r = 1.0 - (v - 0.2) / 0.2;
        g = 1.0;
    } else if v < 0.6 {
        g = 1.0;
        b = (v - 0.4) / 0.2;
    } else if v < 0.8 {
        g = 1.0 - (v - 0.6) / 0.2;
        b = 1.0;
    } else {
        r = (v - 0.8) / 0.2;
        b = 1.0;
    }
    (r, g, b)
}

fn hot_color(v: f32) -> (f32, f32, f32) {
    if v < 0.33 {
        (v / 0.33, 0.0, 0.0)
    } else if v < 0.66 {
        (1.0, (v - 0.33) / 0.33, 0.0)
    } else {
        (1.0, 1.0, (v - 0.66) / 0.34)
    }
}

/// Compute vertex colors for a scalar field
/// If global_min and global_max are provided, they are used for normalization.
/// Otherwise, min/max are computed from the values.
pub fn compute_colors(values: &[f32], scheme: &ColorScheme) -> Vec<f32> {
    compute_colors_with_range(values, scheme, None, None)
}

/// Compute vertex colors with explicit global min/max for consistent normalization across multiple datasets
pub fn compute_colors_with_range(
    values: &[f32],
    scheme: &ColorScheme,
    global_min: Option<f32>,
    global_max: Option<f32>,
) -> Vec<f32> {
    if values.is_empty() {
        return Vec::new();
    }

    let (min, max) = match (global_min, global_max) {
        (Some(gmin), Some(gmax)) => (gmin, gmax),
        _ => {
            // Find min/max using finite values only
            let mut min: Option<f32> = None;
            let mut max: Option<f32> = None;
            for &v in values.iter() {
                if !v.is_finite() {
                    continue;
                }
                min = Some(match min {
                    Some(current) => current.min(v),
                    None => v,
                });
                max = Some(match max {
                    Some(current) => current.max(v),
                    None => v,
                });
            }

            match (min, max) {
                (Some(min), Some(max)) => (min, max),
                _ => {
                    // No finite values; return black
                    return vec![0.0; values.len() * 3];
                }
            }
        }
    };

    let mut range = max - min;
    if !range.is_finite() || range <= 0.0 {
        range = 1.0;
    }

    // Generate colors
    let mut colors = Vec::with_capacity(values.len() * 3);
    for &v in values.iter() {
        let mut normalized = if v.is_finite() {
            (v - min) / range
        } else {
            0.0
        };
        if !normalized.is_finite() {
            normalized = 0.0;
        }
        let (r, g, b) = map_value_to_color(normalized, scheme);
        colors.push(r);
        colors.push(g);
        colors.push(b);
    }

    colors
}

/// Compute field statistics
#[allow(dead_code)]
pub struct FieldStats {
    pub min: f32,
    pub max: f32,
    pub mean: f32,
    pub std_dev: f32,
}

#[allow(dead_code)]
pub fn compute_field_stats(values: &[f32]) -> FieldStats {
    if values.is_empty() {
        return FieldStats {
            min: 0.0,
            max: 0.0,
            mean: 0.0,
            std_dev: 0.0,
        };
    }

    let mut min = values[0];
    let mut max = values[0];
    let mut sum = 0.0;

    for &v in values.iter() {
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
        sum += v;
    }

    let mean = sum / values.len() as f32;

    let mut sum_squared_diff = 0.0;
    for &v in values.iter() {
        let diff = v - mean;
        sum_squared_diff += diff * diff;
    }
    let std_dev = (sum_squared_diff / values.len() as f32).sqrt();

    FieldStats {
        min,
        max,
        mean,
        std_dev,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plot3d::GridDimensions;

    /// Helper to create a test solution
    fn create_test_solution(size: usize, include_gamma: bool) -> Plot3DSolution {
        let mut rho = Vec::with_capacity(size);
        let mut rhou = Vec::with_capacity(size);
        let mut rhov = Vec::with_capacity(size);
        let mut rhow = Vec::with_capacity(size);
        let mut rhoe = Vec::with_capacity(size);

        // Fill with test data
        for i in 0..size {
            let r = 1.0 + (i as f32) * 0.1;
            rho.push(r);
            rhou.push(0.5 * r);
            rhov.push(0.3 * r);
            rhow.push(0.2 * r);
            rhoe.push(2.5 * r);
        }

        let gamma = if include_gamma {
            Some((0..size).map(|i| 1.4 + (i as f32) * 0.01).collect())
        } else {
            None
        };

        Plot3DSolution {
            grid_index: 0,
            dimensions: GridDimensions { i: 2, j: 2, k: 1 },
            rho,
            rhou,
            rhov,
            rhow,
            rhoe,
            gamma,
            metadata: None,
        }
    }

    #[test]
    fn test_scalar_field_from_str() {
        assert!(matches!(
            ScalarField::from_str("density"),
            Some(ScalarField::Density)
        ));
        assert!(matches!(
            ScalarField::from_str("pressure"),
            Some(ScalarField::Pressure)
        ));
        assert!(matches!(
            ScalarField::from_str("velocity_magnitude"),
            Some(ScalarField::VelocityMagnitude)
        ));
        assert!(ScalarField::from_str("invalid").is_none());
    }

    #[test]
    fn test_compute_density_field() {
        let solution = create_test_solution(4, false);
        let result = compute_scalar_field(&solution, ScalarField::Density);

        assert_eq!(result.len(), 4);
        assert!((result[0] - 1.0).abs() < 1e-6);
        assert!((result[1] - 1.1).abs() < 1e-6);
        assert!((result[2] - 1.2).abs() < 1e-6);
        assert!((result[3] - 1.3).abs() < 1e-6);
    }

    #[test]
    fn test_compute_velocity_magnitude() {
        let solution = create_test_solution(4, false);
        let result = compute_scalar_field(&solution, ScalarField::VelocityMagnitude);

        assert_eq!(result.len(), 4);
        // For point 0: u=0.5, v=0.3, w=0.2 -> |V| = sqrt(0.25 + 0.09 + 0.04) = sqrt(0.38)
        let expected = (0.25_f32 + 0.09 + 0.04).sqrt();
        assert!((result[0] - expected).abs() < 1e-4);
    }

    #[test]
    fn test_compute_pressure_with_gamma() {
        let solution = create_test_solution(4, true);
        let result = compute_scalar_field(&solution, ScalarField::Pressure);

        assert_eq!(result.len(), 4);

        // For point 0: rho=1.0, u=0.5, v=0.3, w=0.2, rhoe=2.5, gamma=1.4
        // KE = 0.5 * 1.0 * (0.25 + 0.09 + 0.04) = 0.19
        // IE = 2.5 - 0.19 = 2.31
        // p = (1.4 - 1.0) * 2.31 = 0.924
        let rho = 1.0_f32;
        let ke = 0.5 * rho * (0.25 + 0.09 + 0.04);
        let ie = 2.5 - ke;
        let expected = (1.4 - 1.0) * ie;
        assert!((result[0] - expected).abs() < 1e-2);
    }

    #[test]
    fn test_compute_pressure_without_gamma() {
        let solution = create_test_solution(4, false);
        let result = compute_scalar_field(&solution, ScalarField::Pressure);

        assert_eq!(result.len(), 4);

        // Should use DEFAULT_GAMMA = 1.4
        let rho = 1.0_f32;
        let ke = 0.5 * rho * (0.25 + 0.09 + 0.04);
        let ie = 2.5 - ke;
        let expected = (1.4 - 1.0) * ie;
        assert!((result[0] - expected).abs() < 1e-2);
    }

    #[test]
    fn test_pressure_with_varying_gamma() {
        let solution = create_test_solution(2, true);
        let result = compute_scalar_field(&solution, ScalarField::Pressure);

        // Points should have different gamma values (1.4 and 1.41)
        // So pressures should be slightly different even with same flow pattern
        assert_ne!(result[0], result[1]);
    }

    #[test]
    fn test_compute_momentum_fields() {
        let solution = create_test_solution(4, false);

        let mom_x = compute_scalar_field(&solution, ScalarField::MomentumX);
        assert!((mom_x[0] - 0.5).abs() < 1e-6);

        let mom_y = compute_scalar_field(&solution, ScalarField::MomentumY);
        assert!((mom_y[0] - 0.3).abs() < 1e-6);

        let mom_z = compute_scalar_field(&solution, ScalarField::MomentumZ);
        assert!((mom_z[0] - 0.2).abs() < 1e-6);
    }

    #[test]
    fn test_compute_energy_field() {
        let solution = create_test_solution(4, false);
        let result = compute_scalar_field(&solution, ScalarField::Energy);

        assert_eq!(result.len(), 4);
        assert!((result[0] - 2.5).abs() < 1e-6);
        assert!((result[3] - 3.25).abs() < 1e-5);
    }

    #[test]
    fn test_zero_density_handling() {
        let mut solution = create_test_solution(2, false);
        solution.rho[0] = 0.0;

        let velocity = compute_scalar_field(&solution, ScalarField::VelocityMagnitude);
        assert_eq!(velocity[0], 0.0);

        let pressure = compute_scalar_field(&solution, ScalarField::Pressure);
        assert_eq!(pressure[0], 0.0);
    }

    #[test]
    fn test_compute_field_stats() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let stats = compute_field_stats(&values);

        assert_eq!(stats.min, 1.0);
        assert_eq!(stats.max, 5.0);
        assert_eq!(stats.mean, 3.0);
        assert!((stats.std_dev - 1.4142).abs() < 0.001);
    }

    #[test]
    fn test_field_stats_single_value() {
        let values = vec![42.0];
        let stats = compute_field_stats(&values);

        assert_eq!(stats.min, 42.0);
        assert_eq!(stats.max, 42.0);
        assert_eq!(stats.mean, 42.0);
        assert_eq!(stats.std_dev, 0.0);
    }

    #[test]
    fn test_field_stats_uniform_values() {
        let values = vec![3.14, 3.14, 3.14, 3.14];
        let stats = compute_field_stats(&values);

        assert_eq!(stats.min, 3.14);
        assert_eq!(stats.max, 3.14);
        assert_eq!(stats.mean, 3.14);
        assert!((stats.std_dev).abs() < 1e-6);
    }

    #[test]
    fn test_map_value_to_color_bounds() {
        // Test clamping
        let (r, g, b) = map_value_to_color(-0.5, &ColorScheme::Viridis);
        assert!(r >= 0.0 && r <= 1.0);
        assert!(g >= 0.0 && g <= 1.0);
        assert!(b >= 0.0 && b <= 1.0);

        let (r, g, b) = map_value_to_color(1.5, &ColorScheme::Viridis);
        assert!(r >= 0.0 && r <= 1.0);
        assert!(g >= 0.0 && g <= 1.0);
        assert!(b >= 0.0 && b <= 1.0);
    }

    #[test]
    fn test_map_value_to_color_range() {
        // Test typical values
        let (r0, g0, b0) = map_value_to_color(0.0, &ColorScheme::Viridis);
        let (r1, g1, b1) = map_value_to_color(1.0, &ColorScheme::Viridis);

        // Colors should be different at extremes
        assert!(
            (r0 - r1).abs() > 0.1 || (g0 - g1).abs() > 0.1 || (b0 - b1).abs() > 0.1,
            "Colors at 0 and 1 should be visibly different"
        );
    }

    #[test]
    fn test_compute_colors() {
        let solution = create_test_solution(4, false);
        let field_values = compute_scalar_field(&solution, ScalarField::Density);
        let colors = compute_colors(&field_values, &ColorScheme::Viridis);

        // Should have 3 color components (RGB) per point
        assert_eq!(colors.len(), 4 * 3);

        // All values should be in [0, 1]
        for &c in &colors {
            assert!(c >= 0.0 && c <= 1.0, "Color value {} out of range", c);
        }
    }

    #[test]
    fn test_compute_colors_empty() {
        let values: Vec<f32> = vec![];
        let colors = compute_colors(&values, &ColorScheme::Viridis);
        assert_eq!(colors.len(), 0);
    }

    #[test]
    fn test_color_scheme_from_str() {
        assert!(matches!(
            ColorScheme::from_str("viridis"),
            Some(ColorScheme::Viridis)
        ));
        assert!(matches!(
            ColorScheme::from_str("turbo"),
            Some(ColorScheme::Turbo)
        ));
        assert!(matches!(
            ColorScheme::from_str("rainbow"),
            Some(ColorScheme::Rainbow)
        ));
        assert!(matches!(
            ColorScheme::from_str("hot"),
            Some(ColorScheme::Hot)
        ));
        assert!(matches!(
            ColorScheme::from_str("grayscale"),
            Some(ColorScheme::Grayscale)
        ));
        assert!(ColorScheme::from_str("invalid").is_none());
        assert!(ColorScheme::from_str("").is_none());
    }

    #[test]
    fn test_color_schemes_different() {
        // Different color schemes should produce different colors for the same value
        let (r1, g1, b1) = map_value_to_color(0.5, &ColorScheme::Viridis);
        let (r2, g2, b2) = map_value_to_color(0.5, &ColorScheme::Rainbow);
        let (r3, g3, b3) = map_value_to_color(0.5, &ColorScheme::Hot);
        let (r4, g4, b4) = map_value_to_color(0.5, &ColorScheme::Turbo);

        // At least most schemes should differ from each other at mid-range
        let mut total_diff = 0.0;
        total_diff += (r1 - r2).abs() + (g1 - g2).abs() + (b1 - b2).abs();
        total_diff += (r1 - r3).abs() + (g1 - g3).abs() + (b1 - b3).abs();
        total_diff += (r1 - r4).abs() + (g1 - g4).abs() + (b1 - b4).abs();

        assert!(
            total_diff > 1.0,
            "Color schemes should be visibly different"
        );
    }

    #[test]
    fn test_grayscale_color_scheme() {
        // Grayscale should have equal R, G, B components
        for v in &[0.0, 0.25, 0.5, 0.75, 1.0] {
            let (r, g, b) = map_value_to_color(*v, &ColorScheme::Grayscale);
            assert!((r - g).abs() < 1e-6);
            assert!((g - b).abs() < 1e-6);
            assert!((r - *v).abs() < 1e-6);
        }
    }

    #[test]
    fn test_rainbow_color_transitions() {
        // Test rainbow transitions through spectrum
        let colors: Vec<_> = vec![0.1, 0.3, 0.5, 0.7, 0.9]
            .iter()
            .map(|&v| map_value_to_color(v, &ColorScheme::Rainbow))
            .collect();

        // All colors should be valid RGB
        for (r, g, b) in colors {
            assert!(r >= 0.0 && r <= 1.0);
            assert!(g >= 0.0 && g <= 1.0);
            assert!(b >= 0.0 && b <= 1.0);
        }
    }

    #[test]
    fn test_compute_colors_with_nan_values() {
        let values = vec![1.0, f32::NAN, 3.0, f32::NAN, 5.0];
        let colors = compute_colors(&values, &ColorScheme::Viridis);

        // Should have 5 * 3 = 15 colors
        assert_eq!(colors.len(), 15);

        // All color values should be valid (NaN inputs produce normalized value 0.0)
        for &c in &colors {
            assert!(c.is_finite());
            assert!(c >= 0.0 && c <= 1.0);
        }
    }

    #[test]
    fn test_compute_colors_with_infinite_values() {
        let values = vec![1.0, f32::INFINITY, 3.0, f32::NEG_INFINITY, 5.0];
        let colors = compute_colors(&values, &ColorScheme::Viridis);

        // Should still produce valid colors
        assert_eq!(colors.len(), 15);
        for &c in &colors {
            assert!(c >= 0.0 && c <= 1.0);
        }
    }

    #[test]
    fn test_compute_colors_uniform_field() {
        // When all values are the same, all should map to the same range
        let values = vec![42.0; 10];
        let colors = compute_colors(&values, &ColorScheme::Viridis);

        assert_eq!(colors.len(), 30);
        // All colors should be the same (middle of the colormap since they're uniform)
        for i in 0..10 {
            assert_eq!(colors[i * 3], colors[0]);
            assert_eq!(colors[i * 3 + 1], colors[1]);
            assert_eq!(colors[i * 3 + 2], colors[2]);
        }
    }

    #[test]
    fn test_compute_field_stats_with_negatives() {
        let values = vec![-5.0, -2.0, 0.0, 2.0, 5.0];
        let stats = compute_field_stats(&values);

        assert_eq!(stats.min, -5.0);
        assert_eq!(stats.max, 5.0);
        assert_eq!(stats.mean, 0.0);
        // Using population std dev: sqrt(sum((x - mean)^2) / n)
        // sum of squares: 25 + 4 + 0 + 4 + 25 = 58
        // std_dev = sqrt(58 / 5) = sqrt(11.6) ≈ 3.404
        assert!((stats.std_dev - 3.404).abs() < 0.01);
    }

    #[test]
    fn test_field_stats_large_numbers() {
        let values = vec![1e6, 2e6, 3e6, 4e6, 5e6];
        let stats = compute_field_stats(&values);

        assert_eq!(stats.min, 1e6);
        assert_eq!(stats.max, 5e6);
        assert_eq!(stats.mean, 3e6);
    }

    fn create_test_grid_for_solution(solution: &Plot3DSolution) -> Plot3DGrid {
        use crate::plot3d::{GridDimensions, Plot3DGrid};
        let ni = solution.dimensions.i as usize;
        let nj = solution.dimensions.j as usize;
        let nk = solution.dimensions.k as usize;
        let n = ni * nj * nk;
        let mut x = Vec::with_capacity(n);
        let mut y = Vec::with_capacity(n);
        let mut z = Vec::with_capacity(n);
        for k in 0..nk {
            for j in 0..nj {
                for i in 0..ni {
                    x.push(i as f32);
                    y.push(j as f32);
                    z.push(k as f32);
                }
            }
        }
        Plot3DGrid {
            dimensions: GridDimensions {
                i: ni as u32,
                j: nj as u32,
                k: nk as u32,
            },
            x_coords: x,
            y_coords: y,
            z_coords: z,
            iblank: None,
        }
    }

    #[test]
    fn test_compute_scalar_field_surface() {
        let solution = create_test_solution(18, false);
        let grid = create_test_grid_for_solution(&solution);
        let decimation = 2;

        let result = compute_scalar_field_surface_with_grid(
            &solution,
            &grid,
            ScalarField::Density,
            decimation,
        );

        assert!(!result.is_empty());
        for &v in &result {
            assert!(v.is_finite());
            assert!(v > 0.0);
        }
    }

    #[test]
    fn test_compute_scalar_field_surface_all_fields() {
        let solution = create_test_solution(18, true);
        let grid = create_test_grid_for_solution(&solution);

        let fields = vec![
            ScalarField::Density,
            ScalarField::VelocityMagnitude,
            ScalarField::Pressure,
            ScalarField::MomentumX,
            ScalarField::MomentumY,
            ScalarField::MomentumZ,
            ScalarField::Energy,
        ];

        for field in fields {
            let result = compute_scalar_field_surface_with_grid(&solution, &grid, field, 1);
            assert!(!result.is_empty());
            for &v in &result {
                assert!(v.is_finite(), "Field {:?} produced non-finite value", field);
            }
        }
    }

    #[test]
    fn test_compute_scalar_field_surface_decimation() {
        let solution = create_test_solution(18, false);
        let grid = create_test_grid_for_solution(&solution);

        let result_no_decimation =
            compute_scalar_field_surface_with_grid(&solution, &grid, ScalarField::Density, 1);
        let result_decimation =
            compute_scalar_field_surface_with_grid(&solution, &grid, ScalarField::Density, 2);

        assert!(result_decimation.len() <= result_no_decimation.len());
        assert!(!result_no_decimation.is_empty());
        assert!(!result_decimation.is_empty());
    }

    #[test]
    fn test_map_value_to_color_nan() {
        // NaN should map to black
        let (r, g, b) = map_value_to_color(f32::NAN, &ColorScheme::Viridis);
        assert_eq!(r, 0.0);
        assert_eq!(g, 0.0);
        assert_eq!(b, 0.0);
    }

    #[test]
    fn test_map_value_to_color_infinite() {
        // Infinity should clamp to valid color
        let (r1, g1, b1) = map_value_to_color(f32::INFINITY, &ColorScheme::Viridis);
        assert!(r1 >= 0.0 && r1 <= 1.0);
        assert!(g1 >= 0.0 && g1 <= 1.0);
        assert!(b1 >= 0.0 && b1 <= 1.0);

        let (r2, g2, b2) = map_value_to_color(f32::NEG_INFINITY, &ColorScheme::Viridis);
        assert!(r2 >= 0.0 && r2 <= 1.0);
        assert!(g2 >= 0.0 && g2 <= 1.0);
        assert!(b2 >= 0.0 && b2 <= 1.0);
    }

    #[test]
    fn test_turbo_color_bounds() {
        // Turbo should always produce valid RGB even with edge values
        for &v in &[0.0, 0.25, 0.5, 0.75, 1.0] {
            let (r, g, b) = map_value_to_color(v, &ColorScheme::Turbo);
            assert!(r >= 0.0 && r <= 1.0, "Red out of bounds at v={}", v);
            assert!(g >= 0.0 && g <= 1.0, "Green out of bounds at v={}", v);
            assert!(b >= 0.0 && b <= 1.0, "Blue out of bounds at v={}", v);
        }
    }

    #[test]
    fn test_hot_color_gradient() {
        // Hot color should transition: black -> red -> yellow -> white
        let black = map_value_to_color(0.0, &ColorScheme::Hot);
        let red = map_value_to_color(0.16, &ColorScheme::Hot);
        let yellow = map_value_to_color(0.5, &ColorScheme::Hot);
        let white = map_value_to_color(1.0, &ColorScheme::Hot);

        // Red should be brightest at red point
        assert!(red.0 > black.0);

        // Yellow should have R and G
        assert!(yellow.0 > 0.5);
        assert!(yellow.1 > 0.5);

        // White should be bright in all channels
        assert!(white.0 > 0.9);
        assert!(white.1 > 0.9);
        assert!(white.2 > 0.9);
    }

    // ── Equation tests for new scalar fields ──────────────────────────────────

    /// Return a single-point solution with fully specified primitive state.
    /// γ=1.4, ρ=1.2, u=0.5, v=0.3, w=0.0, total-energy set so p=0.5
    /// p = (γ-1)*(ρe - ½ρV²)  →  ρe = p/(γ-1) + ½ρV²
    fn point_solution(rho: f32, u: f32, v: f32, w: f32, p: f32, gamma: f32) -> Plot3DSolution {
        let v2 = u * u + v * v + w * w;
        let rhoe = p / (gamma - 1.0) + 0.5 * rho * v2;
        Plot3DSolution {
            grid_index: 0,
            dimensions: GridDimensions { i: 1, j: 1, k: 1 },
            rho: vec![rho],
            rhou: vec![rho * u],
            rhov: vec![rho * v],
            rhow: vec![rho * w],
            rhoe: vec![rhoe],
            gamma: Some(vec![gamma]),
            metadata: None,
        }
    }

    #[test]
    fn temperature_equals_p_over_rho() {
        // T = p/ρ (PLOT3D non-dimensional)
        let sol = point_solution(1.2, 0.5, 0.3, 0.0, 0.5, 1.4);
        let t = compute_scalar_field(&sol, ScalarField::Temperature);
        let expected = 0.5_f32 / 1.2;
        assert!(
            (t[0] - expected).abs() < 1e-5,
            "T={} expected={}",
            t[0],
            expected
        );
    }

    #[test]
    fn mach_number_formula() {
        // M = |V|/c,  c = sqrt(γp/ρ)
        let (rho, u, v, w, p, gamma) = (1.2_f32, 0.5, 0.3, 0.0, 0.5, 1.4_f32);
        let sol = point_solution(rho, u, v, w, p, gamma);
        let mach = compute_scalar_field(&sol, ScalarField::MachNumber);
        let vmag = (u * u + v * v).sqrt();
        let c = (gamma * p / rho).sqrt();
        let expected = vmag / c;
        assert!(
            (mach[0] - expected).abs() < 1e-5,
            "M={} expected={}",
            mach[0],
            expected
        );
    }

    #[test]
    fn speed_of_sound_formula() {
        let (rho, u, v, w, p, gamma) = (1.2_f32, 0.5, 0.3, 0.0, 0.5, 1.4_f32);
        let sol = point_solution(rho, u, v, w, p, gamma);
        let c_field = compute_scalar_field(&sol, ScalarField::SpeedOfSound);
        let expected = (gamma * p / rho).sqrt();
        assert!((c_field[0] - expected).abs() < 1e-5);
    }

    #[test]
    fn stagnation_pressure_subsonic_isentropic() {
        // p0 = p * (1 + (γ-1)/2 * M²)^(γ/(γ-1))
        let (rho, u, v, w, p, gamma) = (1.2_f32, 0.3, 0.0, 0.0, 0.5, 1.4_f32);
        let sol = point_solution(rho, u, v, w, p, gamma);
        let p0_field = compute_scalar_field(&sol, ScalarField::StagnationPressure);
        let c = (gamma * p / rho).sqrt();
        let mach = u / c;
        let base = 1.0 + (gamma - 1.0) / 2.0 * mach * mach;
        let expected = p * base.powf(gamma / (gamma - 1.0));
        assert!(
            (p0_field[0] - expected).abs() < 1e-4,
            "p0={} expected={}",
            p0_field[0],
            expected
        );
    }

    #[test]
    fn stagnation_temperature_formula() {
        // T0 = T * (1 + (γ-1)/2 * M²)
        let (rho, u, v, w, p, gamma) = (1.2_f32, 0.3, 0.0, 0.0, 0.5, 1.4_f32);
        let sol = point_solution(rho, u, v, w, p, gamma);
        let t0_field = compute_scalar_field(&sol, ScalarField::StagnationTemperature);
        let c = (gamma * p / rho).sqrt();
        let mach = u / c;
        let t = p / rho;
        let expected = t * (1.0 + (gamma - 1.0) / 2.0 * mach * mach);
        assert!((t0_field[0] - expected).abs() < 1e-4);
    }

    #[test]
    fn stagnation_density_formula() {
        // ρ0 = ρ * (1 + (γ-1)/2 * M²)^(1/(γ-1))
        let (rho, u, v, w, p, gamma) = (1.2_f32, 0.3, 0.0, 0.0, 0.5, 1.4_f32);
        let sol = point_solution(rho, u, v, w, p, gamma);
        let rho0_field = compute_scalar_field(&sol, ScalarField::StagnationDensity);
        let c = (gamma * p / rho).sqrt();
        let mach = u / c;
        let base = 1.0 + (gamma - 1.0) / 2.0 * mach * mach;
        let expected = rho * base.powf(1.0 / (gamma - 1.0));
        assert!((rho0_field[0] - expected).abs() < 1e-4);
    }

    #[test]
    fn pressure_coefficient_zero_at_freestream_mach() {
        // At freestream conditions (M=M∞, ρ=1, p=p∞=1/γ), Cp should be 0.
        let gamma = 1.4_f32;
        let minf = 0.8_f32;
        let p_inf = 1.0 / gamma;
        let c_inf = 1.0_f32; // c∞=1 in PLOT3D non-dim
        let u = minf * c_inf; // u = M∞
        let rho = 1.0_f32;
        let sol = point_solution(rho, u, 0.0, 0.0, p_inf, gamma);
        let cp = compute_scalar_field(&sol, ScalarField::PressureCoefficient);
        // Cp = (p - p∞) / (½ρ∞V∞²) = 0 since p == p_inf at freestream
        assert!(
            cp[0].abs() < 1e-4,
            "Cp should be 0 at freestream, got {}",
            cp[0]
        );
    }

    #[test]
    fn pitot_pressure_subsonic_equals_stagnation_pressure() {
        // Below M=1, pitot pressure IS the isentropic stagnation pressure
        let (rho, u, v, w, p, gamma) = (1.2_f32, 0.3, 0.0, 0.0, 0.5, 1.4_f32);
        let sol = point_solution(rho, u, v, w, p, gamma);
        let pitot = compute_scalar_field(&sol, ScalarField::PitotPressure);
        let p0 = compute_scalar_field(&sol, ScalarField::StagnationPressure);
        assert!(
            (pitot[0] - p0[0]).abs() < 1e-5,
            "pitot={} p0={}",
            pitot[0],
            p0[0]
        );
    }

    #[test]
    fn pitot_pressure_supersonic_less_than_stagnation_pressure() {
        // Above M=1, normal shock reduces total pressure, so pitot < p0_isentropic
        let (rho, u, v, w, p, gamma) = (1.0_f32, 1.5, 0.0, 0.0, 1.0 / 1.4, 1.4_f32);
        // c = sqrt(γp/ρ) = sqrt(1.4*(1/1.4)/1) = 1.0, so M = 1.5
        let sol = point_solution(rho, u, v, w, p, gamma);
        let pitot = compute_scalar_field(&sol, ScalarField::PitotPressure);
        let p0 = compute_scalar_field(&sol, ScalarField::StagnationPressure);
        assert!(
            pitot[0] < p0[0],
            "supersonic pitot ({}) should be < isentropic p0 ({})",
            pitot[0],
            p0[0]
        );
    }

    #[test]
    fn dynamic_pressure_formula() {
        // q = ½ρV²
        let (rho, u, v, w, p, gamma) = (1.2_f32, 0.5, 0.3, 0.0, 0.5, 1.4_f32);
        let sol = point_solution(rho, u, v, w, p, gamma);
        let q_field = compute_scalar_field(&sol, ScalarField::DynamicPressure);
        let v2 = u * u + v * v + w * w;
        let expected = 0.5 * rho * v2;
        assert!((q_field[0] - expected).abs() < 1e-5);
    }

    #[test]
    fn entropy_measure_s1_is_zero_at_isentropic_freestream() {
        // S1 = γp/ρ^γ - 1.  At freestream (p=1/γ, ρ=1): γ*(1/γ)/1^γ - 1 = 0.
        let gamma = 1.4_f32;
        let sol = point_solution(1.0, 0.5, 0.0, 0.0, 1.0 / gamma, gamma);
        let s1 = compute_scalar_field(&sol, ScalarField::EntropyMeasureS1);
        assert!(
            s1[0].abs() < 1e-5,
            "S1 should be 0 at reference state, got {}",
            s1[0]
        );
    }

    #[test]
    fn internal_energy_plus_ke_equals_total_energy() {
        // e_i = e0 - ½V²   →   e_i + ½V² = e0
        let (rho, u, v, w, p, gamma) = (1.2_f32, 0.5, 0.3, 0.0, 0.5, 1.4_f32);
        let sol = point_solution(rho, u, v, w, p, gamma);
        let ei = compute_scalar_field(&sol, ScalarField::InternalEnergy);
        let ke = compute_scalar_field(&sol, ScalarField::KineticEnergy);
        let e0 = compute_scalar_field(&sol, ScalarField::StagnationEnergy);
        assert!(
            (ei[0] + ke[0] - e0[0]).abs() < 1e-4,
            "ei+KE={} e0={}",
            ei[0] + ke[0],
            e0[0]
        );
    }

    #[test]
    fn cross_flow_velocity_ignores_axial_component() {
        // Cross-flow = sqrt(v² + w²), independent of u
        let sol = point_solution(1.0, 10.0, 0.3, 0.4, 1.0 / 1.4, 1.4);
        let cf = compute_scalar_field(&sol, ScalarField::CrossFlowVelocity);
        let expected = (0.3_f32 * 0.3 + 0.4 * 0.4).sqrt();
        assert!((cf[0] - expected).abs() < 1e-5);
    }

    #[test]
    fn log_normalized_density_equals_ln_density() {
        let (rho, u, v, w, p, gamma) = (1.5_f32, 0.3, 0.0, 0.0, 0.5, 1.4_f32);
        let sol = point_solution(rho, u, v, w, p, gamma);
        let ln_rho = compute_scalar_field(&sol, ScalarField::LogNormalizedDensity);
        assert!((ln_rho[0] - rho.ln()).abs() < 1e-5);
    }
}
