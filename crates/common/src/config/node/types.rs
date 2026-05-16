use nalgebra::Vector3;
use serde::{Deserialize, Serialize};

use crate::body::materials::Medium;
use crate::{Body, RevolutionMesh, ThickMesh};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct NodeComputedDebug {
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
    pub frequencies_hz: Vec<f64>,
    pub mode_shapes: Vec<ModeShape>,
    pub strike_damping_in_air: Vec<f64>,
    pub strike_damping_in_medium: Vec<f64>,
    pub jet_damping_in_air: Vec<f64>,
    pub jet_damping_in_medium: Vec<f64>,
    pub medium: Medium,
}

pub type BowlGeometry = ThickMesh<RevolutionMesh<5>>;

pub type ClapperGeometry = RevolutionMesh<3>;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Node {
    pub bowl: Body<BowlGeometry>,
    pub clapper: Body<ClapperGeometry>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ModeShape {
    pub vertex_displacement: Vec<Vector3<f64>>,
    pub frequency_hz: f64,
    pub paths: ModeInteractionPaths,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ModeInteractionPaths {
    pub strike: ModeStrikeAcoustics,
    pub jet: ModeJetAcoustics,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ModeStrikeAcoustics {
    pub coupling: f64,
    pub angle_sensitivity: f64,
    pub damping_in_air: f64,
    pub impact_bandwidth_hz: f64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ModeJetAcoustics {
    pub coupling: f64,
    pub damping_in_air: f64,
    pub lock_center_hz: f64,
    pub lock_bandwidth_hz: f64,
    pub threshold_drive: f64,
    pub small_signal_gain: f64,
    pub convective_delay_s: f64,
    pub strouhal_target: f64,
    pub phase_sensitivity: f64,
    pub radiation_efficiency: f64,
}
