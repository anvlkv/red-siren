use serde::{Deserialize, Serialize};

use crate::NodeKey;

#[derive(Debug, PartialEq, Clone, Copy, Default, Serialize, Deserialize)]
pub struct SensorData {
    pub key: NodeKey,
    pub min_frequency: f32, // Hz
    pub max_frequency: f32, // Hz
    pub min_magnitude: f32, // 0.0 scaled
    pub max_magnitude: f32, // 1.0 scaled
}

impl SensorData {
    pub fn probes(&self, frequency_resolution: f32) -> Vec<f32> {
        let mut probes = Vec::new();
        let mut freq = self.min_frequency;
        while freq <= self.max_frequency {
            probes.push(freq);
            freq += frequency_resolution;
        }
        probes
    }
}
