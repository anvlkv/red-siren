use nalgebra::Vector3;
use serde::{Deserialize, Serialize};

use crate::body::materials::Medium;
use crate::{Body, RevolutionMesh, ThickMesh};

/// Structural properties of the node (FEM-computed, medium-independent).
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct NodeComputedStructure {
    pub requested_resolution: usize,
    pub analysis_resolution: usize,
    pub mode_budget: usize,
    pub mesh_vertex_count: usize,
    pub mesh_triangle_count: usize,
    pub constrained_vertex_count: usize,
    pub active_vertex_count: usize,
    pub bowl_radius_m: f64,
    pub bowl_thickness_m: f64,
    pub bowl_areal_density_kg_per_m2: f64,
    pub bowl_flexural_rigidity: f64,
    pub bowl_surface_area_m2: f64,
    pub bowl_volume_m3: f64,
    pub bowl_mass_kg: f64,
    pub clapper_mass_kg: f64,
    pub clapper_mass_ratio: f64,
    pub solver_total_lumped_mass_kg: f64,
    pub solver_characteristic_edge_length_m: f64,
    pub solver_lambda_min_raw: f64,
    pub solver_lambda_max_raw: f64,
    pub solver_lambda_min_kept: f64,
    pub solver_lambda_max_kept: f64,
    pub solver_dropped_non_finite: usize,
    pub solver_dropped_non_positive: usize,
    pub solver_dropped_near_rigid: usize,
    pub frequencies_hz: Vec<f64>,
    pub mode_structures: Vec<ModeStructure>,
}

/// Acoustic properties of the node (medium-dependent, recomputable).
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct NodeComputedAcoustics {
    pub frequencies_hz: Vec<f64>,
    pub mode_acoustics: Vec<ModeInteractionAcoustics>,
    pub strike_damping_in_air: Vec<f64>,
    pub strike_damping_in_medium: Vec<f64>,
    pub jet_damping_in_air: Vec<f64>,
    pub jet_damping_in_medium: Vec<f64>,
    pub medium: Medium,
}

/// Kept for compatibility; combines structure and acoustics.
pub type NodeComputedDebug = (NodeComputedStructure, NodeComputedAcoustics);

pub type BowlGeometry = ThickMesh<RevolutionMesh<5>>;

pub type ClapperGeometry = RevolutionMesh<3>;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Node {
    pub bowl: Body<BowlGeometry>,
    pub clapper: Body<ClapperGeometry>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ModeStructure {
    pub frequency_hz: f64,
    pub vertex_displacement: Vec<Vector3<f64>>,
    pub strike_base: StrikeStructuralBase,
    pub jet_base: JetStructuralBase,
}

/// Vortex shedding dynamics for jet excitation.
#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct JetVortexDynamics {
    /// Strouhal number for vortex frequency scaling
    pub strouhal_target: f64,
    /// Convective delay through jet [s]
    pub convective_delay_s: f64,
    /// Minimum jet velocity to initiate lock-in
    pub threshold_drive: f64,
    /// Small-signal amplification gain at lock-in
    pub small_signal_gain: f64,
}

/// Structural (medium-independent) properties of strike excitation.
#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct StrikeStructuralBase {
    /// Energy transfer efficiency from impact to mode vibration [0, 1]
    pub coupling: f64,
    /// Directional sensitivity to impact angle
    pub angle_sensitivity: f64,
}

/// Structural (medium-independent) properties of jet excitation.
#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct JetStructuralBase {
    /// Energy transfer efficiency from jet to mode vibration [0, 1]
    pub coupling: f64,
    /// Vortex shedding dynamics
    pub vortex_dynamics: JetVortexDynamics,
}

/// Acoustic properties of strike excitation (medium-dependent, computed on-demand).
#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct StrikeAcousticsInMedium {
    /// Damping due to air viscosity [0, 1]
    pub damping_in_air: f64,
    /// Resonant bandwidth of impact response [Hz]
    pub impact_bandwidth_hz: f64,
}

/// Acoustic properties of jet excitation (medium-dependent, computed on-demand).
#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct JetAcousticsInMedium {
    /// Damping due to air viscosity [0, 1]
    pub damping_in_air: f64,
    /// Acoustic cavity resonance (quarter-wave lock-in)
    pub acoustic_lock_in: AcousticLockIn,
    /// Acoustic radiation efficiency
    pub radiation_efficiency: f64,
}

/// Complete acoustic response for both excitation paths (medium-dependent).
#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct ModeInteractionAcoustics {
    pub strike: StrikeAcousticsInMedium,
    pub jet: JetAcousticsInMedium,
}

/// Acoustic cavity resonance (quarter-wave lock-in).
#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct AcousticLockIn {
    /// Blended structural + acoustic resonance frequency [Hz]
    pub lock_center_hz: f64,
    /// Hysteresis bandwidth around lock-in [Hz]
    pub lock_bandwidth_hz: f64,
    /// Phase alignment sensitivity to frequency deviation
    pub phase_sensitivity: f64,
}
