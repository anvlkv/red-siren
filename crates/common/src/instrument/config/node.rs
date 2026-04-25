use serde::{Deserialize, Serialize};

use crate::instrument::consts::*;
use crate::{error::InstrumentConfigError, NodeKey};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NodeConfig {
    // key
    /// Unique node identifier within the instrument
    pub key: NodeKey,
    // osc
    /// Harmonic frequency of the node
    pub frequency: f64,
    /// Starting phase of the oscillator
    pub phase: f64,
    // meta
    /// cents offset from the base frequency (for fine-tuning)
    pub cents: f64,
    // animalistics
    /// vocal tract length to node in mm (used for formant calculations in physical modeling)
    pub l_mm: f64,
    /// mass of the node in kg (for physical modeling)
    pub w_kg: f64,
    /// volume of the node  cm^3 (for physical modeling)
    pub v_cm3: f64,
}

impl Eq for NodeConfig {}

impl PartialOrd for NodeConfig {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NodeConfig {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.key.cmp(&other.key)
    }
}

impl NodeConfig {
    /// Unified node validation.
    /// Order:
    /// 1. Structural (range shape)
    /// 2. Recommended (soft) bounds (emit *recommended* errors)
    /// 3. Safe (hard) bounds (emit *safe* errors)
    pub(crate) fn validate(&self, idx: usize) -> Result<(), InstrumentConfigError> {
        // 1. Structural
        // 2. Recommended bounds (soft limits)
        if self.frequency < SOFT_MIN_FREQ_HZ {
            return Err(InstrumentConfigError::NodeFreqencyBelowRecomended {
                node: idx,
                freq: self.frequency as f32,
            });
        }
        if self.frequency > SOFT_MAX_FREQ_HZ {
            return Err(InstrumentConfigError::NodeFreqencyAboveRecomended {
                node: idx,
                freq: self.frequency as f32,
            });
        }

        // 3. Safe bounds (hard limits) - only reached if recommended passed.
        if self.frequency < MIN_FREQ_HZ {
            return Err(InstrumentConfigError::NodeFreqencyBelowSafe {
                node: idx,
                freq: self.frequency as f32,
            });
        }
        if self.frequency > MAX_FREQ_HZ {
            return Err(InstrumentConfigError::NodeFreqencyAboveSafe {
                node: idx,
                freq: self.frequency as f32,
            });
        }

        Ok(())
    }

    /// Calculate the formant frequency for a given formant number (1-based index) based on the vocal tract length of the node.
    pub fn formant_hz(&self, formant: usize) -> f64 {
        ((2.0 * formant as f64 - 1.0) * SPEED_OF_SOUND_M_S) / (4.0 * (self.l_mm / 1000.0))
    }

    /// Volume of the node in cubic meters (SI unit for buoyancy and displacement calculations)
    pub fn v_m3(&self) -> f64 {
        self.v_cm3 / 1_000_000.0
    }

    /// Calculate the heart rate in beats per minute based on the mass of the node using an allometric scaling law.
    pub fn hr_bpm(&self) -> u16 {
        (241.0 * self.w_kg.powf(-0.25)).ceil() as u16
    }

    /// Calculate the buoyant force exerted on the node when submerged in a fluid with the given density (in g/cm³). The force is returned in Newtons.
    pub fn buoyant_force(&self, fluid_density_g_cm3: f64) -> f64 {
        self.v_cm3 * fluid_density_g_cm3 * GRAVITY_M_S2
    }

    /// Calculate the body density of the node in g/cm³.
    pub fn body_density_g_cm3(&self) -> f64 {
        (self.w_kg * 1000.0) / self.v_cm3
    }

    #[cfg(any(test, feature = "test"))]
    pub fn new_test_node(f: f64) -> Self {
        static NODE_KEY: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
        let key = NODE_KEY.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        Self {
            key: NodeKey(0, key),
            frequency: f,
            phase: 0.0,
            cents: 100.0,
            l_mm: 170.0,
            w_kg: 0.1,
            v_cm3: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::instrument::config::config_test_cases;

    use insta::assert_json_snapshot;

    #[test]
    fn test_computed_values() {
        config_test_cases()
            .map(|(config, layout)| {
                let space = layout.space;
                let data = config
                    .0
                    .into_iter()
                    .flat_map(|g| g.nodes)
                    .map(|node| {
                        let formants =
                            Vec::from_iter((1..=5).map(|f| (format!("F {f}"), node.formant_hz(f))));
                        let hr_bpm = node.hr_bpm();
                        let buoyant_force = node.buoyant_force(1.0);
                        let body_density = node.body_density_g_cm3();

                        (
                            format!("key: {:?}", node.key),
                            format!("freq: {}Hz", node.frequency),
                            format!("cents: {} cents", node.cents),
                            format!("l_mm: {} mm", node.l_mm),
                            format!("w_kg: {} kg", node.w_kg),
                            format!("v_cm3: {} cm³", node.v_cm3),
                            format!("hr: {} bpm", hr_bpm),
                            format!("buoyant force: {} N", buoyant_force),
                            format!("body density: {} g/cm³", body_density),
                            formants,
                        )
                    })
                    .collect::<Vec<_>>();
                (space, data)
            })
            .for_each(|(space, data)| {
                assert_json_snapshot!(format!("config_{}x{}", space.x, space.y), data);
            });
    }
}
