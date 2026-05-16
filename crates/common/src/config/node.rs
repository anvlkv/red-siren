use nalgebra::{Point3, Vector3};
use serde::{Deserialize, Serialize};

use crate::{config::Band, Body, RevolutionMesh, ThickMesh};

pub type BowlGeometry = ThickMesh<RevolutionMesh<5>>;

pub type ClapperGeometry = RevolutionMesh<3>;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Node {
    pub bowl: Body<BowlGeometry>,
    pub clapper: Body<ClapperGeometry>,
}

impl Node {
    pub fn new(bowl: Body<BowlGeometry>, clapper: Body<ClapperGeometry>) -> Self {
        Self { bowl, clapper }
    }

    pub fn modal_frequencies_hz(&self, resolution: usize) -> Vec<f64> {
        todo!()
    }

    pub fn mode_shapes(&self, resolution: usize) -> Vec<ModeShape> {
        todo!()
    }

    pub fn modal_participation_factor(
        &self,
        point: Point3<f64>,
        direction: Vector3<f64>,
        resolution: usize,
    ) -> Vec<f64> {
        todo!()
    }

    pub fn modal_damping(&self, band: &Band, resolution: usize) -> Vec<f64> {
        todo!()
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ModeShape {
    pub vertex_displacement: Vec<Vector3<f64>>,
    pub frequency_hz: f64,
    pub lock_in_range_hz: f64,
    pub angle_sensitivity: f64,
    pub strike_coupling: f64,
    pub jet_coupling: f64,
    pub radiation_efficiency: f64,
    pub damping: f64,
}
