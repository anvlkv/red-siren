use serde::{Deserialize, Serialize};

use crate::instrument::consts::*;
use crate::{error::InstrumentConfigError, NodeKey};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Unique node identifier within the instrument
    pub key: NodeKey,
    /// Harmonic frequency of the node
    pub frequency: f64,
    /// vocal tract length to node in mm
    pub l: f64,
    /// Starting phase of the oscillator
    pub phase: f64,
    /// Number of divisions
    pub divisions: u32,
    /// cents
    pub cents: f64,
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

    pub fn formant_hz(&self, formant: usize) -> f64 {
        ((2.0 * formant as f64 - 1.0) * SPEED_OF_SOUND_M_S) / (4.0 * (self.l / 1000.0))
    }

    #[cfg(any(test, feature = "test"))]
    pub fn new_test_node(f: f64) -> Self {
        static NODE_KEY: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
        let key = NODE_KEY.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        Self {
            key: NodeKey(0, key),
            frequency: f,
            l: 170.0,
            phase: 0.1,
            divisions: 7,
            cents: f / 1200.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::instrument::config::config_test_cases;

    use insta::assert_json_snapshot;

    #[test]
    fn test_formants() {
        let data = config_test_cases()
            .map(|(config, layout)| {
                let space = layout.space;
                let data = config
                    .0
                    .into_iter()
                    .flat_map(|g| g.nodes)
                    .map(|node| {
                        let formants =
                            Vec::from_iter((1..=5).map(|f| (format!("F{f}"), node.formant_hz(f))));

                        (
                            format!("{:?}", node.key),
                            format!("{}Hz", node.frequency),
                            format!("{} cents", node.cents),
                            formants,
                        )
                    })
                    .collect::<Vec<_>>();
                (space, data)
            })
            .collect::<Vec<_>>();

        assert_json_snapshot!(data);
    }
}
