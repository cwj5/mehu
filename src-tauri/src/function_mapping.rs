/// Legacy PLOT3D FUNCTION number mapping.
///
/// This module translates legacy integer function IDs into canonical
/// `ScalarField` values. It is intentionally explicit and deterministic.
///
/// IMPORTANT:
/// - We do not guess equations.
/// - Known legacy functions without implemented equations are marked
///   `known_unimplemented` and return a warning diagnostic.
/// - Unknown or out-of-scope IDs soft-fail with a warning diagnostic.
use crate::plot_state::{cap, Diagnostic, ScalarField};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyFunctionStatus {
    Supported,
    KnownUnimplemented,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyFunctionEntry {
    pub number: u16,
    pub scalar_field: ScalarField,
    pub label: &'static str,
    pub status: LegacyFunctionStatus,
    /// Human-edit placeholder for equation/definition work still needed.
    pub equation_todo: Option<&'static str>,
}

const LEGACY_SCALAR_FUNCTIONS: &[LegacyFunctionEntry] = &[
    // Implemented now
    LegacyFunctionEntry {
        number: 100,
        scalar_field: ScalarField::Density,
        label: "Density (or Q1)",
        status: LegacyFunctionStatus::Supported,
        equation_todo: None,
    },
    LegacyFunctionEntry {
        number: 110,
        scalar_field: ScalarField::Pressure,
        label: "Pressure",
        status: LegacyFunctionStatus::Supported,
        equation_todo: None,
    },
    LegacyFunctionEntry {
        number: 150,
        scalar_field: ScalarField::UVelocity,
        label: "u velocity",
        status: LegacyFunctionStatus::Supported,
        equation_todo: None,
    },
    LegacyFunctionEntry {
        number: 151,
        scalar_field: ScalarField::VVelocity,
        label: "v velocity",
        status: LegacyFunctionStatus::Supported,
        equation_todo: None,
    },
    LegacyFunctionEntry {
        number: 152,
        scalar_field: ScalarField::WVelocity,
        label: "w velocity",
        status: LegacyFunctionStatus::Supported,
        equation_todo: None,
    },
    LegacyFunctionEntry {
        number: 153,
        scalar_field: ScalarField::VelocityMagnitude,
        label: "Velocity magnitude",
        status: LegacyFunctionStatus::Supported,
        equation_todo: None,
    },
    LegacyFunctionEntry {
        number: 160,
        scalar_field: ScalarField::MomentumX,
        label: "x-momentum (Q2)",
        status: LegacyFunctionStatus::Supported,
        equation_todo: None,
    },
    LegacyFunctionEntry {
        number: 161,
        scalar_field: ScalarField::MomentumY,
        label: "y-momentum (Q3)",
        status: LegacyFunctionStatus::Supported,
        equation_todo: None,
    },
    LegacyFunctionEntry {
        number: 162,
        scalar_field: ScalarField::MomentumZ,
        label: "z-momentum (Q4)",
        status: LegacyFunctionStatus::Supported,
        equation_todo: None,
    },
    LegacyFunctionEntry {
        number: 163,
        scalar_field: ScalarField::Energy,
        label: "Stagnation energy per unit volume (Q5)",
        status: LegacyFunctionStatus::Supported,
        equation_todo: None,
    },
    // Known IDs, intentionally not computed yet (no guessed equations)
    LegacyFunctionEntry {
        number: 101,
        scalar_field: ScalarField::NormalizedDensity,
        label: "Normalized density",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: normalized density reference definition"),
    },
    LegacyFunctionEntry {
        number: 102,
        scalar_field: ScalarField::StagnationDensity,
        label: "Stagnation density",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: stagnation density"),
    },
    LegacyFunctionEntry {
        number: 103,
        scalar_field: ScalarField::NormalizedStagnationDensity,
        label: "Normalized stagnation density",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: normalized stagnation density reference"),
    },
    LegacyFunctionEntry {
        number: 104,
        scalar_field: ScalarField::LogNormalizedDensity,
        label: "Log of normalized density",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: log(normalized density)"),
    },
    LegacyFunctionEntry {
        number: 111,
        scalar_field: ScalarField::NormalizedPressure,
        label: "Normalized pressure",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: normalized pressure reference"),
    },
    LegacyFunctionEntry {
        number: 112,
        scalar_field: ScalarField::StagnationPressure,
        label: "Stagnation pressure",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: stagnation pressure"),
    },
    LegacyFunctionEntry {
        number: 113,
        scalar_field: ScalarField::NormalizedStagnationPressure,
        label: "Normalized stagnation pressure",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: normalized stagnation pressure"),
    },
    LegacyFunctionEntry {
        number: 114,
        scalar_field: ScalarField::PressureCoefficient,
        label: "Pressure coefficient",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: Cp with reference conditions"),
    },
    LegacyFunctionEntry {
        number: 115,
        scalar_field: ScalarField::StagnationPressureCoefficient,
        label: "Stagnation pressure coefficient",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: stagnation pressure coefficient"),
    },
    LegacyFunctionEntry {
        number: 116,
        scalar_field: ScalarField::PitotPressure,
        label: "Pitot pressure",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: pitot pressure"),
    },
    LegacyFunctionEntry {
        number: 117,
        scalar_field: ScalarField::PitotPressureRatio,
        label: "Pitot pressure ratio",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: pitot pressure ratio"),
    },
    LegacyFunctionEntry {
        number: 118,
        scalar_field: ScalarField::DynamicPressure,
        label: "Dynamic pressure",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: dynamic pressure"),
    },
    LegacyFunctionEntry {
        number: 119,
        scalar_field: ScalarField::LogNormalizedPressure,
        label: "Log of normalized pressure",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: log(normalized pressure)"),
    },
    LegacyFunctionEntry {
        number: 120,
        scalar_field: ScalarField::Temperature,
        label: "Temperature",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: temperature"),
    },
    LegacyFunctionEntry {
        number: 121,
        scalar_field: ScalarField::NormalizedTemperature,
        label: "Normalized temperature",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: normalized temperature"),
    },
    LegacyFunctionEntry {
        number: 122,
        scalar_field: ScalarField::StagnationTemperature,
        label: "Stagnation temperature",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: stagnation temperature"),
    },
    LegacyFunctionEntry {
        number: 123,
        scalar_field: ScalarField::NormalizedStagnationTemperature,
        label: "Normalized stagnation temperature",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: normalized stagnation temperature"),
    },
    LegacyFunctionEntry {
        number: 124,
        scalar_field: ScalarField::LogNormalizedTemperature,
        label: "Log of normalized temperature",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: log(normalized temperature)"),
    },
    LegacyFunctionEntry {
        number: 130,
        scalar_field: ScalarField::Enthalpy,
        label: "Enthalpy",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: enthalpy"),
    },
    LegacyFunctionEntry {
        number: 131,
        scalar_field: ScalarField::NormalizedEnthalpy,
        label: "Normalized enthalpy",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: normalized enthalpy"),
    },
    LegacyFunctionEntry {
        number: 132,
        scalar_field: ScalarField::StagnationEnthalpy,
        label: "Stagnation enthalpy",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: stagnation enthalpy"),
    },
    LegacyFunctionEntry {
        number: 133,
        scalar_field: ScalarField::NormalizedStagnationEnthalpy,
        label: "Normalized stagnation enthalpy",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: normalized stagnation enthalpy"),
    },
    LegacyFunctionEntry {
        number: 140,
        scalar_field: ScalarField::InternalEnergy,
        label: "(Internal) energy",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: internal energy"),
    },
    LegacyFunctionEntry {
        number: 141,
        scalar_field: ScalarField::NormalizedInternalEnergy,
        label: "Normalized (internal) energy",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: normalized internal energy"),
    },
    LegacyFunctionEntry {
        number: 142,
        scalar_field: ScalarField::StagnationEnergy,
        label: "Stagnation energy",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: stagnation energy"),
    },
    LegacyFunctionEntry {
        number: 143,
        scalar_field: ScalarField::NormalizedStagnationEnergy,
        label: "Normalized stagnation energy",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: normalized stagnation energy"),
    },
    LegacyFunctionEntry {
        number: 144,
        scalar_field: ScalarField::KineticEnergy,
        label: "Kinetic energy",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: kinetic energy"),
    },
    LegacyFunctionEntry {
        number: 145,
        scalar_field: ScalarField::NormalizedKineticEnergy,
        label: "Normalized kinetic energy",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: normalized kinetic energy"),
    },
    LegacyFunctionEntry {
        number: 154,
        scalar_field: ScalarField::MachNumber,
        label: "Mach number",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: Mach number"),
    },
    LegacyFunctionEntry {
        number: 155,
        scalar_field: ScalarField::SpeedOfSound,
        label: "Speed of sound",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: speed of sound"),
    },
    LegacyFunctionEntry {
        number: 156,
        scalar_field: ScalarField::CrossFlowVelocity,
        label: "Cross-flow velocity",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: cross-flow velocity"),
    },
    LegacyFunctionEntry {
        number: 157,
        scalar_field: ScalarField::Normalized2dStreamFunction,
        label: "Normalized 2D stream function",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: normalized 2D stream function"),
    },
    LegacyFunctionEntry {
        number: 158,
        scalar_field: ScalarField::VelocityDivergence,
        label: "Divergence of velocity",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: velocity divergence"),
    },
    LegacyFunctionEntry {
        number: 170,
        scalar_field: ScalarField::Entropy,
        label: "Entropy",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: entropy"),
    },
    LegacyFunctionEntry {
        number: 171,
        scalar_field: ScalarField::EntropyMeasureS1,
        label: "Entropy measure s1",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: entropy measure s1"),
    },
    LegacyFunctionEntry {
        number: 180,
        scalar_field: ScalarField::VorticityX,
        label: "x-component of vorticity",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: vorticity x"),
    },
    LegacyFunctionEntry {
        number: 181,
        scalar_field: ScalarField::VorticityY,
        label: "y-component of vorticity",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: vorticity y"),
    },
    LegacyFunctionEntry {
        number: 182,
        scalar_field: ScalarField::VorticityZ,
        label: "z-component of vorticity",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: vorticity z"),
    },
    LegacyFunctionEntry {
        number: 183,
        scalar_field: ScalarField::VorticityMagnitude,
        label: "Vorticity magnitude",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: vorticity magnitude"),
    },
    LegacyFunctionEntry {
        number: 184,
        scalar_field: ScalarField::Swirl,
        label: "Swirl",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: swirl"),
    },
    LegacyFunctionEntry {
        number: 185,
        scalar_field: ScalarField::VelocityCrossVorticityMagnitude,
        label: "Velocity x vorticity magnitude",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: |V x omega|"),
    },
    LegacyFunctionEntry {
        number: 186,
        scalar_field: ScalarField::HelicityDensity,
        label: "Helicity density",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: helicity density"),
    },
    LegacyFunctionEntry {
        number: 187,
        scalar_field: ScalarField::RelativeHelicity,
        label: "Relative helicity",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: relative helicity"),
    },
    LegacyFunctionEntry {
        number: 188,
        scalar_field: ScalarField::FilteredRelativeHelicity,
        label: "Filtered relative helicity",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: filtered relative helicity"),
    },
    LegacyFunctionEntry {
        number: 190,
        scalar_field: ScalarField::ShockFunctionPressureGradient,
        label: "Shock function based on pressure gradient",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: shock function"),
    },
    LegacyFunctionEntry {
        number: 191,
        scalar_field: ScalarField::FilteredShockFunction,
        label: "Filtered shock function",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: filtered shock function"),
    },
    LegacyFunctionEntry {
        number: 192,
        scalar_field: ScalarField::PressureGradientMagnitude,
        label: "Pressure gradient magnitude",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: |grad p|"),
    },
    LegacyFunctionEntry {
        number: 193,
        scalar_field: ScalarField::DensityGradientMagnitude,
        label: "Density gradient magnitude",
        status: LegacyFunctionStatus::KnownUnimplemented,
        equation_todo: Some("TODO_EQUATION: |grad rho|"),
    },
];

pub fn legacy_scalar_entry(number: u16) -> Option<&'static LegacyFunctionEntry> {
    LEGACY_SCALAR_FUNCTIONS
        .iter()
        .find(|entry| entry.number == number)
}

/// Map a legacy FUNCTION number into a currently-computable `ScalarField`.
///
/// Returns:
/// - `Some(field)` with no diagnostics for currently supported entries.
/// - `None` + warning diagnostic for known-but-unimplemented entries.
/// - `None` + warning diagnostic for non-scalar/unsupported ranges and unknown IDs.
pub fn map_legacy_function_number(number: u16) -> (Option<ScalarField>, Vec<Diagnostic>) {
    if let Some(entry) = legacy_scalar_entry(number) {
        return match entry.status {
            LegacyFunctionStatus::Supported => (Some(entry.scalar_field), Vec::new()),
            LegacyFunctionStatus::KnownUnimplemented => {
                let todo = entry
                    .equation_todo
                    .unwrap_or("TODO_EQUATION: define formula");
                (
                    None,
                    vec![Diagnostic::warning(
                        cap::FUNCTION,
                        format!(
                            "FUNCTION {} ({}) is recognized but not implemented yet: {}",
                            entry.number, entry.label, todo
                        ),
                    )],
                )
            }
        };
    }

    if number < 100 {
        return (
            None,
            vec![Diagnostic::warning(
                cap::FUNCTION,
                format!(
                    "FUNCTION {} is a grid-function ID (0-99) and is not supported by scalar mapping",
                    number
                ),
            )],
        );
    }

    if number >= 200 && number <= 299 {
        return (
            None,
            vec![Diagnostic::warning(
                cap::FUNCTION,
                format!(
                    "FUNCTION {} is a vector-function ID (200-299) and is out of current scalar scope",
                    number
                ),
            )],
        );
    }

    (
        None,
        vec![Diagnostic::warning(
            cap::FUNCTION,
            format!(
                "FUNCTION {} is unknown or out of current scope; command ignored",
                number
            ),
        )],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_supported_values_map_deterministically() {
        let (field_100, diags_100) = map_legacy_function_number(100);
        let (field_110, diags_110) = map_legacy_function_number(110);
        let (field_153, diags_153) = map_legacy_function_number(153);
        let (field_160, diags_160) = map_legacy_function_number(160);
        let (field_163, diags_163) = map_legacy_function_number(163);

        assert_eq!(field_100, Some(ScalarField::Density));
        assert_eq!(field_110, Some(ScalarField::Pressure));
        assert_eq!(field_153, Some(ScalarField::VelocityMagnitude));
        assert_eq!(field_160, Some(ScalarField::MomentumX));
        assert_eq!(field_163, Some(ScalarField::Energy));

        assert!(diags_100.is_empty());
        assert!(diags_110.is_empty());
        assert!(diags_153.is_empty());
        assert!(diags_160.is_empty());
        assert!(diags_163.is_empty());
    }

    #[test]
    fn known_unimplemented_values_soft_fail_with_warning() {
        let (field, diags) = map_legacy_function_number(154);
        assert_eq!(field, None);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].capability, cap::FUNCTION);
        assert!(diags[0].message.contains("recognized but not implemented"));
        assert!(diags[0].message.contains("TODO_EQUATION"));
    }

    #[test]
    fn grid_function_values_soft_fail_with_warning() {
        let (field, diags) = map_legacy_function_number(10);
        assert_eq!(field, None);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("grid-function"));
    }

    #[test]
    fn vector_function_values_soft_fail_with_warning() {
        let (field, diags) = map_legacy_function_number(210);
        assert_eq!(field, None);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("vector-function"));
    }

    #[test]
    fn unknown_values_soft_fail_with_warning() {
        let (field, diags) = map_legacy_function_number(999);
        assert_eq!(field, None);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("unknown or out of current scope"));
    }

    #[test]
    fn entry_lookup_exposes_label_for_gui_or_parser_usage() {
        let entry = legacy_scalar_entry(114).expect("entry 114 should exist");
        assert_eq!(entry.label, "Pressure coefficient");
        assert_eq!(entry.status, LegacyFunctionStatus::KnownUnimplemented);
    }
}
