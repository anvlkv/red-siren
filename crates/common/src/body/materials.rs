use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Material {
    /// Stable source identifier for lookup/catalog joins.
    pub id: String,
    /// Density from CSV `Ro` at `reference_temperature_c` in kg/m^3.
    pub reference_density_kg_per_m3: f64,
    /// Poisson ratio from CSV `mu`.
    /// This model treats it as temperature-invariant.
    pub poisson_ratio: f64,
    /// Young's modulus from CSV `E` at `reference_temperature_c` in MPa.
    pub reference_youngs_modulus_mpa: f64,
    /// Reference temperature for tabulated material properties.
    pub reference_temperature_c: f64,
    /// Linear thermal expansion coefficient [1/C].
    pub linear_thermal_expansion_per_c: f64,
    /// Log-slope of Young's modulus with temperature [1/C]: d(ln E)/dT.
    /// Negative values mean the material softens with heat.
    pub dln_e_dtemp_per_c: f64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Medium {
    /// Stable source identifier for lookup/catalog joins.
    pub source_id: String,
    /// Human-readable substance name.
    pub substance: String,
    /// Thermodynamic phase label, e.g. "gas" or "liquid".
    pub phase: String,
    /// Ambient temperature in degrees Celsius.
    pub temperature_c: f64,
    /// Ambient pressure in Pascals.
    pub pressure_pa: f64,

    /// Density snapshot in kg/m^3.
    pub density_kg_per_m3: f64,
    /// Speed-of-sound snapshot in m/s.
    pub speed_of_sound_m_per_s: f64,
    /// Dynamic viscosity snapshot in Pa*s.
    pub viscosity_pa_s: f64,
    /// Acoustic impedance snapshot in Rayl.
    pub impedance_m_rayl: f64,
}

impl Medium {
    pub const STANDARD_TEMPERATURE_C: f64 = 20.0;
    pub const STANDARD_PRESSURE_PA: f64 = 101_325.0;

    pub fn standard_air() -> Self {
        Self {
            source_id: "STANDARD_AIR".to_string(),
            substance: "Air".to_string(),
            phase: "gas".to_string(),
            temperature_c: Self::STANDARD_TEMPERATURE_C,
            pressure_pa: Self::STANDARD_PRESSURE_PA,
            density_kg_per_m3: 1.2041,
            speed_of_sound_m_per_s: 343.0,
            viscosity_pa_s: 1.81e-5,
            impedance_m_rayl: 1.2041 * 343.0,
        }
    }

    pub fn impedance_from_density_and_speed(
        density_kg_per_m3: f64,
        speed_of_sound_m_per_s: f64,
    ) -> f64 {
        density_kg_per_m3.max(0.0) * speed_of_sound_m_per_s.max(0.0)
    }

    /// Helper that fills a snapshot from explicitly available properties.
    /// If impedance is missing in source data, pass `None` to compute `rho * c`.
    pub fn from_available(
        source_id: &str,
        substance: &str,
        phase: &str,
        temperature_c: f64,
        pressure_pa: f64,
        density_kg_per_m3: f64,
        speed_of_sound_m_per_s: f64,
        viscosity_pa_s: f64,
        impedance_m_rayl: Option<f64>,
    ) -> Self {
        Self {
            source_id: source_id.to_string(),
            substance: substance.to_string(),
            phase: phase.to_string(),
            temperature_c,
            pressure_pa,
            density_kg_per_m3,
            speed_of_sound_m_per_s,
            viscosity_pa_s,
            impedance_m_rayl: impedance_m_rayl.unwrap_or_else(|| {
                Self::impedance_from_density_and_speed(density_kg_per_m3, speed_of_sound_m_per_s)
            }),
        }
    }
}

impl Default for Medium {
    fn default() -> Self {
        Self::standard_air()
    }
}

impl Material {
    /// Reference temperature assumed for all tabulated CSV property values.
    pub const REFERENCE_TEMPERATURE_C: f64 = 20.0;

    /// Build a `Material` from the fields that are always present in the CSV:
    /// `E` (Young's modulus in MPa), `mu` (Poisson ratio), `Ro` (density kg/m³).
    ///
    /// `reference_temperature_c` is the temperature at which the tabulated values
    /// were measured. Pass `None` to use `REFERENCE_TEMPERATURE_C` (20 °C).
    ///
    /// Temperature-response coefficients are absent from the CSV; pass `None` to
    /// leave them at zero (no temperature correction until data is available).
    pub fn from_available(
        id: &str,
        reference_youngs_modulus_mpa: f64,
        poisson_ratio: f64,
        reference_density_kg_per_m3: f64,
        reference_temperature_c: Option<f64>,
        linear_thermal_expansion_per_c: Option<f64>,
        dln_e_dtemp_per_c: Option<f64>,
    ) -> Self {
        Self {
            id: id.to_string(),
            reference_youngs_modulus_mpa,
            poisson_ratio,
            reference_density_kg_per_m3,
            reference_temperature_c: reference_temperature_c
                .unwrap_or(Self::REFERENCE_TEMPERATURE_C),
            linear_thermal_expansion_per_c: linear_thermal_expansion_per_c.unwrap_or(0.0),
            dln_e_dtemp_per_c: dln_e_dtemp_per_c.unwrap_or(0.0),
        }
    }

    /// Derive Poisson ratio from Young's and shear moduli (isotropic material).
    /// Useful when CSV `mu` is absent but `E` and `G` are both present.
    pub fn poisson_ratio_from_e_and_g(e_mpa: f64, g_mpa: f64) -> f64 {
        (e_mpa / (2.0 * g_mpa.max(f64::EPSILON))) - 1.0
    }

    pub fn youngs_modulus_pa_at_temperature_c(&self, temperature_c: f64) -> f64 {
        let delta_c = temperature_c - self.reference_temperature_c;
        let scale = (1.0 + self.dln_e_dtemp_per_c * delta_c).max(0.05);
        (self.reference_youngs_modulus_mpa * 1.0e6 * scale).max(1.0e6)
    }

    pub fn density_kg_per_m3_at_temperature_c(&self, temperature_c: f64) -> f64 {
        let delta_c = temperature_c - self.reference_temperature_c;
        let volumetric_expansion = 3.0 * self.linear_thermal_expansion_per_c * delta_c;
        (self.reference_density_kg_per_m3 / (1.0 + volumetric_expansion).max(0.1)).max(1.0)
    }

    pub fn reference_youngs_modulus_pa(&self) -> f64 {
        self.reference_youngs_modulus_mpa * 1.0e6
    }
}

#[cfg(test)]
mod tests {
    use super::{Material, Medium};

    #[test]
    fn medium_helpers_compute_impedance_from_available_properties() {
        let medium = Medium::from_available(
            "C7732185", "Water", "liquid", 20.0, 101_325.0, 998.2, 1482.4, 1.0014e-3, None,
        );
        let expected = 998.2 * 1482.4;
        assert!((medium.impedance_m_rayl - expected).abs() <= expected * 1e-12);
        assert_eq!(medium.source_id, "C7732185");
        assert_eq!(medium.substance, "Water");
        assert_eq!(medium.phase, "liquid");

        let overridden = Medium::from_available(
            "C7732185",
            "Water",
            "liquid",
            20.0,
            101_325.0,
            998.2,
            1482.4,
            1.0014e-3,
            Some(1.0),
        );
        assert_eq!(overridden.impedance_m_rayl, 1.0);
    }

    #[test]
    fn medium_snapshot_is_explicit_not_implicit() {
        let medium = Medium {
            source_id: "TEST_MEDIUM".to_string(),
            substance: "Synthetic Test Medium".to_string(),
            phase: "gas".to_string(),
            temperature_c: 20.0,
            pressure_pa: 101_325.0,
            density_kg_per_m3: 2.0,
            speed_of_sound_m_per_s: 120.0,
            viscosity_pa_s: 8.0e-4,
            impedance_m_rayl: 999.0,
        };

        assert_eq!(medium.density_kg_per_m3, 2.0);
        assert_eq!(medium.speed_of_sound_m_per_s, 120.0);
        assert_eq!(medium.viscosity_pa_s, 8.0e-4);
        assert_eq!(medium.impedance_m_rayl, 999.0);
        assert_eq!(medium.phase, "gas");
    }

    #[test]
    fn material_from_available_csv_row_steel_sae_1015() {
        // CSV: E=207000, G=79000, mu=0.3, Ro=7860 (Steel SAE 1015, as-rolled)
        let m = Material::from_available("D8894772", 207_000.0, 0.3, 7860.0, None, None, None);
        assert_eq!(m.id, "D8894772");
        assert_eq!(m.reference_youngs_modulus_mpa, 207_000.0);
        assert_eq!(m.poisson_ratio, 0.3);
        assert_eq!(m.reference_density_kg_per_m3, 7860.0);
        assert_eq!(m.reference_temperature_c, Material::REFERENCE_TEMPERATURE_C);
        assert_eq!(m.linear_thermal_expansion_per_c, 0.0);
        assert_eq!(m.dln_e_dtemp_per_c, 0.0);
        // With no temperature coefficients, modulus stays constant regardless of temp.
        assert!(
            (m.youngs_modulus_pa_at_temperature_c(100.0) - m.reference_youngs_modulus_pa()).abs()
                < 1.0
        );
    }

    #[test]
    fn material_poisson_ratio_from_e_and_g_matches_csv_mu() {
        // CSV: E=207000, G=79000, mu=0.3
        // Isotropic formula: mu = E/(2G) - 1 = 207000/158000 - 1 ≈ 0.3101
        let derived = Material::poisson_ratio_from_e_and_g(207_000.0, 79_000.0);
        assert!(derived > 0.29 && derived < 0.32);
    }

    #[test]
    fn material_temperature_response_is_finite_and_reversible_at_reference() {
        let steel_like = Material {
            id: "STEEL_LIKE".to_string(),
            reference_density_kg_per_m3: 7860.0,
            poisson_ratio: 0.30,
            reference_youngs_modulus_mpa: 207_000.0,
            reference_temperature_c: 20.0,
            linear_thermal_expansion_per_c: 12.0e-6,
            dln_e_dtemp_per_c: -4.0e-4,
        };

        let e_ref =
            steel_like.youngs_modulus_pa_at_temperature_c(steel_like.reference_temperature_c);
        assert!((e_ref - steel_like.reference_youngs_modulus_pa()).abs() < 1e-6);

        let e_hot = steel_like.youngs_modulus_pa_at_temperature_c(100.0);
        let e_cold = steel_like.youngs_modulus_pa_at_temperature_c(-20.0);
        assert!(e_hot < e_ref);
        assert!(e_cold > e_ref);

        let rho_hot = steel_like.density_kg_per_m3_at_temperature_c(100.0);
        assert!(rho_hot.is_finite() && rho_hot > 0.0);
    }
}
