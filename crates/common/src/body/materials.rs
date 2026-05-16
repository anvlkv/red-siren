use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct Material {
    /// Density of the material in kg/m^3.
    pub density_kg_per_m3: f64,
    /// Poison's ratio of the material, describes how much a material narrows sideways when stretched.
    pub poisson_ratio: f64,
    /// Young's modulus of the material in Pascals, describes the stiffness of the material.
    pub youngs_modulus_pa: f64,
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct Medium {
    /// Density of the medium in kg/m^3, representing the mass per unit volume of the medium.
    ///
    /// Adds density damping and energy loss
    pub density_kg_per_m3: f64,
    /// Speed of sound in the medium in meters per second, representing how quickly sound waves propagate through the medium.
    ///
    /// Governs the relationship between frequency and wavelength, affecting the tonal characteristics.
    pub speed_of_sound_m_per_s: f64,
    /// Viscous damping coefficient in Pascals per second, representing the internal friction of the band material.
    ///
    /// Adds viscous drag that smears high frequencies and shortens decay through shear losses at the surface.
    pub viscosity_pa_s: f64,
    /// Acoustic impedance of the medium in Rayls, representing the resistance to sound wave propagation through the medium.
    ///
    /// Governs energy transfer efficiency.
    pub impedance_m_rayl: f64,
}
