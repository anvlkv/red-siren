use serde::{Deserialize, Serialize};

use crate::error::InstrumentConfigError;

use crate::NodeKey;

use super::{consts::*, Layout};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
/// Instrument configuartion for audio generation
pub struct Config(pub Vec<GroupConfig>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// Output chanel of the group
pub enum GroupChannel {
    Left,
    Right,
}

fn fundamental_frequency(n: usize, v: f64, l: f64) -> f64 {
    (n as f64 * v) / (2.0 * l)
}

impl GroupChannel {
    pub(crate) fn from_keys_groups(k: u32, g: u32) -> Self {
        match (g.is_multiple_of(2), k.is_multiple_of(2)) {
            (false, false) => Self::Left,
            (false, true) => Self::Right,
            (true, false) => Self::Right,
            (true, true) => Self::Left,
        }
    }

    pub fn nth_channel_from_first(&self, n: usize) -> Self {
        if n.is_multiple_of(2) {
            *self
        } else {
            match self {
                Self::Left => Self::Right,
                Self::Right => Self::Left,
            }
        }
    }

    fn compute_fundamentals(&self, l: f32, mut n_base: usize) -> (f64, usize) {
        let v = match self {
            Self::Left => CRIMSON_RED_WAVESPEED,
            Self::Right => CINNABAR_RED_WAVESPEED,
        };

        let mut f: f64 = 0.0;

        while f < SOFT_MIN_FREQ_HZ {
            f = fundamental_frequency(n_base, v, l as f64);
            if f < SOFT_MIN_FREQ_HZ {
                n_base += 1;
            }
        }

        (f, n_base)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// Japanese Scale Interval Patterns
pub enum Scale {
    /// Yo Scale (bright pentatonic)
    ///
    /// Semitone sequence: 2 - 3 - 2 - 2 - 3
    ///
    /// Formula (counted from tonic, C): C, D (+2), F (+5), G (+7), A (+9)
    #[default]
    Yo,
    /// In Scale (dark pentatonic)
    ///
    /// Semitone sequence: 1 - 4 - 1 - 4 - 2
    ///
    /// Formula: C, D♭ (+1), F (+5), G (+7), A♭ (+8)
    In,
}

impl Scale {
    fn semitones_12(&self) -> &'static [u8] {
        match self {
            Scale::Yo => &[0, 2, 5, 7, 9],
            Scale::In => &[0, 1, 5, 7, 8],
        }
    }

    /// Map the 12-TET semitone degrees into indices for `divisions`-EDO.
    /// Rounds to nearest division index; duplicates removed.
    fn mapped_divisions(&self, divisions: u32) -> smallvec::SmallVec<[u32; 8]> {
        use smallvec::SmallVec;
        let mut out: SmallVec<[u32; 8]> = self
            .semitones_12()
            .iter()
            .map(|s| {
                let idx = ((*s as f64) * (divisions as f64) / 12.0).round() as i64;
                idx.clamp(0, divisions as i64 - 1) as u32
            })
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupConfig {
    /// Output channel
    pub channel: GroupChannel,
    /// Group nodes
    pub nodes: Vec<NodeConfig>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Base frequency of the node
    pub base_frequency: f64,
    /// Starting phase of the oscillator
    pub phase: f64,
    /// Unique node identifier within the instrument
    pub key: NodeKey,
}

impl NodeConfig {
    /// Unified node validation.
    /// Order:
    /// 1. Structural (range shape)
    /// 2. Recommended (soft) bounds (emit *recommended* errors)
    /// 3. Safe (hard) bounds (emit *safe* errors)
    fn validate(&self, idx: usize) -> Result<(), InstrumentConfigError> {
        // 1. Structural
        // 2. Recommended bounds (soft limits)
        if self.base_frequency < SOFT_MIN_FREQ_HZ {
            return Err(InstrumentConfigError::NodeFreqencyBelowRecomended {
                node: idx,
                freq: self.base_frequency as f32,
            });
        }
        if self.base_frequency > SOFT_MAX_FREQ_HZ {
            return Err(InstrumentConfigError::NodeFreqencyAboveRecomended {
                node: idx,
                freq: self.base_frequency as f32,
            });
        }

        // 3. Safe bounds (hard limits) - only reached if recommended passed.
        if self.base_frequency < MIN_FREQ_HZ {
            return Err(InstrumentConfigError::NodeFreqencyBelowSafe {
                node: idx,
                freq: self.base_frequency as f32,
            });
        }
        if self.base_frequency > MAX_FREQ_HZ {
            return Err(InstrumentConfigError::NodeFreqencyAboveSafe {
                node: idx,
                freq: self.base_frequency as f32,
            });
        }

        Ok(())
    }
}

impl GroupConfig {
    /// Unified group validation (safe constraints only).
    fn validate(&self, _group_idx: usize) -> Result<(), InstrumentConfigError> {
        if self.nodes.is_empty() {
            return Err(InstrumentConfigError::EmptyGroup);
        }
        for (i, n) in self.nodes.iter().enumerate() {
            n.validate(i)?;
        }
        Ok(())
    }
}

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
}

fn build_group_nodes(
    group_f_base: f64,
    group: usize,
    scale: Scale,
    equal_divisions: u32,
) -> Vec<NodeConfig> {
    // Already mapped division indices (sorted, deduped)
    let divs = scale.mapped_divisions(equal_divisions);
    if divs.is_empty() {
        return Vec::new();
    }

    // Boundaries
    let mut boundaries: Vec<f64> = Vec::with_capacity(divs.len() + 1);
    boundaries.push(0.0);
    for w in divs.windows(2) {
        boundaries.push((w[0] as f64 + w[1] as f64) * 0.5);
    }
    boundaries.push(equal_divisions as f64);

    let mut nodes = Vec::with_capacity(divs.len());
    for (i, &d) in divs.iter().enumerate() {
        let center_ratio = 2f64.powf(d as f64 / equal_divisions as f64);
        let base_f = group_f_base * center_ratio;
        if base_f > MAX_FREQ_HZ {
            continue;
        }

        nodes.push(NodeConfig {
            base_frequency: base_f,
            phase: 0.0,
            key: NodeKey::new(group as u8, i as u8),
        });
    }

    nodes
}

/// Distribute node phases to avoid phase stacking and add stereo width.
///
/// Strategy:
/// - Even spread 0..2π across nodes
/// - Per-group offset so groups are decorrelated
/// - Per-channel offset to widen stereo image
fn assign_node_phases(nodes: &mut [NodeConfig], group_index: usize, channel: GroupChannel) {
    if nodes.is_empty() {
        return;
    }
    let group_offset = (group_index as f64) * std::f64::consts::PI / 3.0;
    let channel_offset = match channel {
        GroupChannel::Left => 0.0,
        GroupChannel::Right => std::f64::consts::FRAC_PI_4,
    };
    let n = nodes.len() as f64;
    for (k, node) in nodes.iter_mut().enumerate() {
        let spread = 2.0 * std::f64::consts::PI * (k as f64) / n;
        node.phase = group_offset + spread + channel_offset;
        node.key = NodeKey::new(group_index as u8, k as u8);
    }
}

impl TryFrom<Layout> for Config {
    type Error = InstrumentConfigError;

    fn try_from(value: Layout) -> Result<Self, Self::Error> {
        let a = value.left_string_position.0;
        let b = value.left_string_position.1;

        let l = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();

        let equal_divisions = value.num_keys_per_group.get() as u32;
        let scale = value.scale;

        let mut groups = Vec::new();

        for (g_x, n) in (0..value.num_groups.get() as usize).map(|g_x| (g_x, 2 * g_x + 1)) {
            let g_channel = value.first_group_channel.nth_channel_from_first(g_x);

            let (group_f_base, _group_n_base) = g_channel.compute_fundamentals(l, n);

            let f_base = group_f_base * 2usize.pow(g_x as u32) as f64;

            let mut nodes = build_group_nodes(f_base, g_x, scale, equal_divisions);

            // Phase spreading
            assign_node_phases(&mut nodes, g_x, g_channel);

            groups.push(GroupConfig {
                channel: g_channel,
                nodes,
            });
        }

        let config = Config(groups);

        // Single-pass validation (recommended + safe)
        // Caller can decide how to surface any error.
        config.validate()?;

        // (previous panic on unsafe config removed in favor of Result error propagation)

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::super::layout_test_cases;
    use super::*;

    #[test]
    fn test_config_from_layout_validity() {
        for layout in layout_test_cases() {
            let config = Config::try_from(layout).expect("Config from layout should be valid");
            // validate() already ran in try_from, but call again defensively
            assert!(
                config.validate().is_ok(),
                "Config from layout should pass unified validation"
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
    fn test_scale_mapped_divisions() {
        let yo = Scale::Yo;
        let in_scale = Scale::In;
        let divs_yo = yo.mapped_divisions(12);
        let divs_in = in_scale.mapped_divisions(12);
        assert_eq!(
            divs_yo,
            smallvec::SmallVec::<[u32; 8]>::from_vec(vec![0u32, 2, 5, 7, 9])
        );
        assert_eq!(
            divs_in,
            smallvec::SmallVec::<[u32; 8]>::from_vec(vec![0u32, 1, 5, 7, 8])
        );
    }

    #[test]
    fn test_node_config_validity() {
        let valid_node = NodeConfig {
            base_frequency: (super::SOFT_MIN_FREQ_HZ + super::SOFT_MAX_FREQ_HZ) / 2.0,
            phase: 0.0,
            key: NodeKey::new(0, 0),
        };
        assert!(valid_node.validate(0).is_ok());
        assert!(NodeConfig {
            base_frequency: super::MIN_FREQ_HZ - 1.0,
            phase: 0.0,
            key: NodeKey::new(0, 0),
        }
        .validate(0)
        .is_err());
    }

    #[test]
    fn test_group_config_validity() {
        let node = NodeConfig {
            base_frequency: (super::SOFT_MIN_FREQ_HZ + super::SOFT_MAX_FREQ_HZ) / 2.0,
            phase: 0.0,
            key: NodeKey::new(0, 0),
        };
        let group = GroupConfig {
            channel: GroupChannel::Left,
            nodes: vec![node],
        };
        assert!(group.validate(0).is_ok());
    }

    #[test]
    fn test_phase_spreading_uniform() {
        // Use first layout to build a config
        let layout = layout_test_cases().next().expect("at least one layout");
        let config = Config::try_from(layout).expect("layout should yield a valid config");

        // Pick the first group that has >= 3 nodes so differences are meaningful
        let group = config
            .0
            .iter()
            .find(|g| g.nodes.len() >= 3)
            .expect("need a group with at least 3 nodes for phase test");

        let phases: Vec<f64> = group.nodes.iter().map(|n| n.phase).collect();

        // All phases must be unique
        {
            let mut sorted = phases.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            sorted.dedup();
            assert_eq!(sorted.len(), phases.len(), "Phases should all be unique");
        }

        // Consecutive phase deltas should be (approximately) constant (2π / N)
        let n = phases.len() as f64;
        let expected_step = 2.0 * std::f64::consts::PI / n;

        // Rebuild expected sequence modulo constant offset; we just check ratios of actual deltas.
        let mut deltas = Vec::new();
        for k in 0..phases.len() {
            let a = phases[k];
            let b = phases[(k + 1) % phases.len()]; // wrap to test final gap
                                                    // Normalize difference to [0, 2π)
            let mut d = b - a;
            while d < 0.0 {
                d += 2.0 * std::f64::consts::PI;
            }
            while d >= 2.0 * std::f64::consts::PI {
                d -= 2.0 * std::f64::consts::PI;
            }
            deltas.push(d);
        }
        // Sort to reduce sensitivity to the wrap gap (there will be exactly one large gap if offset differs)
        deltas.sort_by(|x, y| x.partial_cmp(y).unwrap());
        // Ignore the largest (wrap) gap if present by comparing median
        let median = deltas[deltas.len() / 2];
        let tol = expected_step * 0.05; // 5% tolerance
        assert!(
            (median - expected_step).abs() <= tol,
            "Median phase step {:?} deviates from expected {:?} (tol {:?})",
            median,
            expected_step,
            tol
        );
    }

    #[test]
    fn test_config_max_volume_check() {
        // Construct a config that should violate the max node budget:
        // MAX_DBS is treated as the hard cap on total simultaneous nodes.
        let excessive_nodes = MAX_DBS + 1;
        let dummy_node = NodeConfig {
            base_frequency: (SOFT_MIN_FREQ_HZ + SOFT_MAX_FREQ_HZ) / 2.0,
            phase: 0.0,
            key: NodeKey::new(0, 0),
        };
        let group = GroupConfig {
            channel: GroupChannel::Left,
            nodes: vec![dummy_node.clone(); excessive_nodes],
        };
        let cfg_excess = Config(vec![group]);
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
        let cfg_ok = Config(vec![group_ok]);
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
