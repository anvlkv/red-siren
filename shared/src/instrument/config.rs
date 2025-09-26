use std::ops::Range;

use mint::Point2;
use serde::{Deserialize, Serialize};

use super::Layout;

// Include the generated constants
include!(concat!(env!("OUT_DIR"), "/instrument_constants_gen.rs"));

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// Instrument configuartion for audio generation
pub struct Config(pub Vec<GroupConfig>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// Output chanel of the group
pub enum GroupChanel {
    Left,
    Right,
}

impl GroupChanel {
    pub(crate) fn from_keys_groups(k: u32, g: u32) -> Self {
        match (g % 2, k % 2) {
            (0, 0) => Self::Left,  // even-even: start Left
            (0, 1) => Self::Right, // even-odd: start Right
            (1, 0) => Self::Right, // odd-even: start Right
            (1, 1) => Self::Left,  // odd-odd: start Left
            _ => Self::Left,       // fallback (should not occur)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupConfig {
    /// Output channel
    pub channel: GroupChanel,
    /// Group nodes
    pub nodes: Vec<NodeConfig>,
    /// Controls pause at zero crossings
    pub a_coef: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Base frequency of the node
    pub base_frequency: f32,
    /// Starting phase of the oscillator
    pub phase: f32,
    /// Range in which node frequency may change
    pub band_range: Range<f32>,
}

impl NodeConfig {
    /// Validate whether node resonates in safe frequency range
    fn valid(&self) -> bool {
        self.base_frequency >= MIN_FREQ_HZ
            && self.base_frequency <= MAX_FREQ_HZ
            && self.band_range.start >= MIN_FREQ_HZ
            && self.band_range.end <= MAX_FREQ_HZ
            && self.band_range.start < self.band_range.end
    }

    /// Validate whether node resonates in recommended frequency range
    fn strict_valid(&self) -> bool {
        self.base_frequency >= SOFT_MIN_FREQ_HZ
            && self.base_frequency <= SOFT_MAX_FREQ_HZ
            && self.band_range.start >= SOFT_MIN_FREQ_HZ
            && self.band_range.end <= SOFT_MAX_FREQ_HZ
            && self.band_range.start < self.band_range.end
    }
}

impl GroupConfig {
    /// Validate whether group configuration is valid
    fn valid(&self) -> bool {
        (0.0..1.0).contains(&self.a_coef) && self.nodes.iter().all(|n| n.valid())
    }

    /// Strict validation of group configuration
    fn strict_valid(&self) -> bool {
        (0.0..1.0).contains(&self.a_coef) && self.nodes.iter().all(|n| n.strict_valid())
    }
}

impl Config {
    /// Validate whether all nodes in all groups are valid
    ///
    /// - Resonate in safe frequencies
    /// - Total volume does not exceed max dB
    fn valid(&self) -> bool {
        self.0.iter().all(|g| g.valid()) && (self.0.len() + 1) <= MAX_DBS && self.valid_channels()
    }

    /// Strict validation of all nodes in all groups
    ///
    /// - Resonate in recommended frequencies
    /// - Total volume does not exceed max dB
    fn strict_valid(&self) -> bool {
        self.0.iter().all(|g| g.strict_valid())
            && (self.0.len() + 1) <= MAX_DBS
            && self.valid_channels()
    }

    fn valid_channels(&self) -> bool {
        self.0
            .iter()
            .try_fold(Option::<GroupChanel>::None, |prev, current| {
                if let Some(ch) = prev {
                    if ch != current.channel {
                        Ok(Some(current.channel))
                    } else {
                        Err(())
                    }
                } else {
                    Ok(Some(current.channel))
                }
            })
            .is_ok()
    }
}

impl From<Layout> for Config {
    fn from(value: Layout) -> Self {
        // let v = value.first_group_channel;

        // let a = value.left_string_position.0;
        // let b = value.left_string_position.1;

        // let base_freq = find_base_freq(v, a, b);

        todo!()
    }
}
