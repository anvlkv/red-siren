use serde::{Deserialize, Serialize};

use crate::config::NodePhysics;

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct Band {
    pub f0: f64,
    pub node_increment: NodePhysics,
    pub band_increment: NodePhysics,
    pub channel: super::BandChannel,
}
