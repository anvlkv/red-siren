mod channel;
mod group;
mod node;
mod scale;

use mint::Point2;
use serde::{Deserialize, Serialize};

use crate::{error::InstrumentConfigError, orientation::LayoutOrientation, NodeKey};

use super::{consts::*, Layout};

pub use channel::*;
pub use group::*;
pub use node::*;
pub use scale::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
/// Instrument configuartion for audio generation
pub struct Config(pub Vec<GroupConfig>, pub Scale);

impl Config {
    /// Simultaneous node "power" budget check.
    /// Assumption: a worst-case event drives every node at gain 1.0.
    /// We approximate loudness budget by limiting total node count.
    fn max_event_volume_ok(&self) -> bool {
        let total_nodes: usize = self.0.iter().map(|g| g.nodes.len()).sum();
        total_nodes <= MAX_DBS
    }

    /// Validate whether all nodes in all groups are valid
    ///
    /// - Resonate in safe frequencies
    /// - Total volume does not exceed max dB
    fn validate(&self) -> Result<(), InstrumentConfigError> {
        if self.0.is_empty() {
            return Err(InstrumentConfigError::Empty);
        }

        let all_len = self.0.first().map_or(0, |g| g.nodes.len());

        if self.0.iter().any(|g| g.nodes.len() != all_len) {
            return Err(InstrumentConfigError::InvalidGroups);
        }

        for (gi, g) in self.0.iter().enumerate() {
            g.validate(gi)?;
        }
        if !self.max_event_volume_ok() {
            let total_nodes: usize = self.0.iter().map(|g| g.nodes.len()).sum();
            return Err(InstrumentConfigError::MaxCumulativeGainAboveSafe(
                total_nodes as f32,
            ));
        }
        self.validate_channels()?;
        Ok(())
    }

    /// Validation of all nodes in all groups
    ///
    /// - Resonate in recommended frequencies
    /// - Total volume does not exceed max dB
    fn validate_channels(&self) -> Result<(), InstrumentConfigError> {
        let mut prev: Option<GroupChannel> = None;
        for g in &self.0 {
            if prev == Some(g.channel) {
                return Err(InstrumentConfigError::ChannelsConfigurationInvalid);
            }
            prev = Some(g.channel);
        }
        Ok(())
    }

    pub fn group_nth_channel(
        &self,
        channel: GroupChannel,
        nth_in_channel: usize,
    ) -> Option<&GroupConfig> {
        self.0
            .iter()
            .filter(|g| g.channel == channel)
            .nth(nth_in_channel)
    }

    pub fn channel_of_key(&self, key: &NodeKey) -> Option<GroupChannel> {
        self.0
            .iter()
            .find(|g| g.nodes.iter().any(|n| &n.key == key))
            .map(|g| g.channel)
    }

    pub fn num_groups_left(&self) -> usize {
        self.0
            .iter()
            .filter(|g| matches!(g.channel, GroupChannel::Left))
            .count()
    }

    pub fn num_groups_right(&self) -> usize {
        self.0
            .iter()
            .filter(|g| matches!(g.channel, GroupChannel::Right))
            .count()
    }

    pub fn num_groups(&self) -> usize {
        self.0.len()
    }

    pub fn num_nodes_per_group(&self) -> usize {
        self.0.first().map_or(0, |g| g.nodes.len())
    }

    pub fn num_nodes_total(&self) -> usize {
        self.0.iter().map(|g| g.nodes.len()).sum()
    }

    pub fn min_frequency_hz(&self) -> f64 {
        self.0
            .first()
            .and_then(|g| g.nodes.first())
            .map(|n| n.frequency.min(n.formant_hz(1)))
            .unwrap_or(super::consts::SOFT_MIN_FREQ_HZ)
    }

    pub fn max_frequency_hz(&self) -> f64 {
        self.0
            .last()
            .and_then(|g| g.nodes.last())
            .map(|n| n.frequency.max(n.formant_hz(5)))
            .unwrap_or(super::consts::SOFT_MAX_FREQ_HZ)
    }
}

impl TryFrom<Layout> for Config {
    type Error = InstrumentConfigError;

    fn try_from(layout: Layout) -> Result<Self, Self::Error> {
        let key_registry = layout.registry();

        // physical parameters
        let a = layout.left_string_position.0;
        let b = layout.left_string_position.1;
        let string_len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();

        // initial data
        let num_groups = key_registry.num_groups() as usize;
        let num_divisions_per_group = key_registry.num_keys_per_group() as usize;
        let scale = layout.scale;

        // computed properties
        let total_steps = key_registry.total_keys();
        let phase_step = 1.0 / total_steps as f64;
        let l_step_nodes = layout.key_radius + layout.key_bands_gap;

        // track values
        let mut n = 0;
        let mut n_base = 1;
        let mut l = {
            let p = layout
                .orientation
                .safe_length_start_point(Point2 { x: 0.0, y: 0.0 }, layout.safe_area_padding);
            (match layout.orientation {
                LayoutOrientation::Horizontal => p.x,
                LayoutOrientation::Vertical => p.y,
            }) + layout.key_pad_main()
        };

        let mut groups = Vec::new();
        let mut target_f_min = None;
        for g in 0..num_groups {
            let channel = layout.first_group_channel.nth_channel_from_first(g);

            let (octave_f_base, next_n_base) =
                compute_fundamentals(string_len, n_base, target_f_min);

            let mut nodes = vec![];

            for k in 0..num_divisions_per_group {
                let key = key_registry.create_key(g.try_into()?, k.try_into()?)?;

                let frequency =
                    scale.freq_n(k as f64, octave_f_base, num_divisions_per_group as f64);

                let divisions = num_divisions_per_group as u32;
                let phase = phase_step * n as f64;
                let cents = 1200.0 * (frequency / octave_f_base).log2();

                nodes.push(NodeConfig {
                    key,
                    frequency,
                    l,
                    phase,
                    divisions,
                    cents,
                });

                l += l_step_nodes;
                n += 1;
            }

            l += layout.groups_gap;
            target_f_min = Some(octave_f_base * 2.0);
            n_base = (next_n_base + 1).max(g + 1);

            nodes.sort();

            groups.push(GroupConfig { channel, nodes });
        }

        let config = Config(groups, scale);

        // Single-pass validation (recommended + safe)
        // Caller can decide how to surface any error.
        config.validate()?;

        Ok(config)
    }
}

fn fundamental_frequency(n: usize, v: f64, l: f64) -> f64 {
    (n as f64 * v) / (2.0 * l)
}

fn compute_fundamentals(l: f64, mut n_base: usize, min_freq: Option<f64>) -> (f64, usize) {
    let v = if n_base.is_multiple_of(2) {
        CRIMSON_RED_WAVESPEED
    } else {
        CINNABAR_RED_WAVESPEED
    };

    let mut f: f64 = 0.0;

    let min_target = min_freq.unwrap_or(SOFT_MIN_FREQ_HZ);

    while f < min_target {
        f = fundamental_frequency(n_base, v, l);
        if f < min_target {
            n_base += 1;
        }
    }

    (f, n_base)
}

#[cfg(any(test, feature = "test"))]
pub fn config_test_cases() -> impl Iterator<Item = (Config, Layout)> {
    crate::instrument::layout::layout_test_cases()
        .filter_map(|l| Config::try_from(l).ok().map(|c| (c, l)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instrument::layout::layout_test_cases;
    use insta::assert_json_snapshot;

    #[test]
    fn test_config_from_layout_validity() {
        for layout in layout_test_cases() {
            let config = Config::try_from(layout).expect("Config from layout should be valid");
            assert_json_snapshot!(
                format!(
                    "config_{}x{}_{:?}",
                    layout.space.x, layout.space.y, layout.scale
                ),
                config
            );
        }
    }

    #[test]
    fn test_group_channel_nth_channel_from_first() {
        assert_eq!(
            GroupChannel::Left.nth_channel_from_first(0),
            GroupChannel::Left
        );
        assert_eq!(
            GroupChannel::Left.nth_channel_from_first(1),
            GroupChannel::Right
        );
        assert_eq!(
            GroupChannel::Right.nth_channel_from_first(0),
            GroupChannel::Right
        );
        assert_eq!(
            GroupChannel::Right.nth_channel_from_first(1),
            GroupChannel::Left
        );
    }

    #[test]
    fn test_node_config_validity() {
        let valid_node =
            NodeConfig::new_test_node((super::SOFT_MIN_FREQ_HZ + super::SOFT_MAX_FREQ_HZ) / 2.0);
        assert!(valid_node.validate(0).is_ok());
        assert!(NodeConfig::new_test_node(super::MIN_FREQ_HZ - 1.0)
            .validate(0)
            .is_err());
    }

    #[test]
    fn test_group_config_validity() {
        let node =
            NodeConfig::new_test_node((super::SOFT_MIN_FREQ_HZ + super::SOFT_MAX_FREQ_HZ) / 2.0);
        let group = GroupConfig {
            channel: GroupChannel::Left,
            nodes: vec![node],
        };
        assert!(group.validate(0).is_ok());
    }

    #[test]
    fn test_config_max_volume_check() {
        // Construct a config that should violate the max node budget:
        // MAX_DBS is treated as the hard cap on total simultaneous nodes.
        let excessive_nodes = MAX_DBS + 1;
        let dummy_node = NodeConfig::new_test_node((SOFT_MIN_FREQ_HZ + SOFT_MAX_FREQ_HZ) / 2.0);
        let group = GroupConfig {
            channel: GroupChannel::Left,
            nodes: vec![dummy_node; excessive_nodes],
        };
        let cfg_excess = Config(vec![group], Scale::Yo);
        assert!(
            !cfg_excess.max_event_volume_ok(),
            "Excessive node count should fail volume check"
        );
        assert!(
            cfg_excess.validate().is_err(),
            "Config with excessive nodes should be invalid"
        );

        // Construct a config that is just at the limit
        let ok_nodes = MAX_DBS;
        let group_ok = GroupConfig {
            channel: GroupChannel::Left,
            nodes: vec![dummy_node; ok_nodes],
        };
        let cfg_ok = Config(vec![group_ok], Scale::In);
        assert!(
            cfg_ok.max_event_volume_ok(),
            "Node count at limit should pass"
        );
        // Single group: channel alternation not applicable. Uses recommended band; validate() should succeed.
        assert!(
            cfg_ok.validate().is_ok(),
            "Config at volume limit should remain valid"
        );
    }
}
