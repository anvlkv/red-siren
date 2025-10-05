use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::error::InstrumentConfigError;

use super::{consts::*, Layout};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
/// Instrument configuartion for audio generation
pub struct Config(pub Vec<GroupConfig>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// Output chanel of the group
pub enum GroupChanel {
    Left,
    Right,
}

fn fundamental_frequency(n: usize, v: f64, l: f64) -> f64 {
    (n as f64 * v) / (2.0 * l)
}

impl GroupChanel {
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
    pub channel: GroupChanel,
    /// Group nodes
    pub nodes: Vec<NodeConfig>,
    /// Controls pause at zero crossings
    pub a_coef: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Base frequency of the node
    pub base_frequency: f64,
    /// Starting phase of the oscillator
    pub phase: f64,
    /// Range in which node frequency may change
    pub band_range: Range<f64>,
}

impl NodeConfig {
    /// Unified node validation.
    /// Order:
    /// 1. Structural (range shape)
    /// 2. Recommended (soft) bounds (emit *recommended* errors)
    /// 3. Safe (hard) bounds (emit *safe* errors)
    fn validate(&self, idx: usize) -> Result<(), InstrumentConfigError> {
        // 1. Structural
        if self.band_range.start >= self.band_range.end {
            return Err(InstrumentConfigError::NodeBandRangeInvalid {
                node: idx,
                start: self.band_range.start as f32,
                end: self.band_range.end as f32,
            });
        }

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
        if self.band_range.start < SOFT_MIN_FREQ_HZ {
            return Err(InstrumentConfigError::NodeBandStartBelowRecomended {
                node: idx,
                freq: self.band_range.start as f32,
            });
        }
        if self.band_range.start > SOFT_MAX_FREQ_HZ {
            return Err(InstrumentConfigError::NodeBandStartAboveRecomended {
                node: idx,
                freq: self.band_range.start as f32,
            });
        }
        if self.band_range.end < SOFT_MIN_FREQ_HZ {
            return Err(InstrumentConfigError::NodeBandEndBelowRecomended {
                node: idx,
                freq: self.band_range.end as f32,
            });
        }
        if self.band_range.end > SOFT_MAX_FREQ_HZ {
            return Err(InstrumentConfigError::NodeBandEndAboveRecomended {
                node: idx,
                freq: self.band_range.end as f32,
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
        if self.band_range.start < MIN_FREQ_HZ {
            return Err(InstrumentConfigError::NodeBandStartBelowSafe {
                node: idx,
                freq: self.band_range.start as f32,
            });
        }
        if self.band_range.start > MAX_FREQ_HZ {
            return Err(InstrumentConfigError::NodeBandStartAboveSafe {
                node: idx,
                freq: self.band_range.start as f32,
            });
        }
        if self.band_range.end < MIN_FREQ_HZ {
            return Err(InstrumentConfigError::NodeBandEndBelowSafe {
                node: idx,
                freq: self.band_range.end as f32,
            });
        }
        if self.band_range.end > MAX_FREQ_HZ {
            return Err(InstrumentConfigError::NodeBandEndAboveSafe {
                node: idx,
                freq: self.band_range.end as f32,
            });
        }

        Ok(())
    }
}

impl GroupConfig {
    /// Unified group validation (safe constraints only).
    fn validate(&self, _group_idx: usize) -> Result<(), InstrumentConfigError> {
        if !(0.0..1.0).contains(&self.a_coef) {
            return Err(InstrumentConfigError::InvalidACoef(self.a_coef));
        }
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
        let mut prev: Option<GroupChanel> = None;
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
        channel: GroupChanel,
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
            .filter(|g| matches!(g.channel, GroupChanel::Left))
            .count()
    }

    pub fn num_groups_right(&self) -> usize {
        self.0
            .iter()
            .filter(|g| matches!(g.channel, GroupChanel::Right))
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
    group_n_base: usize,
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

    // Harmonic spacing cap
    let harmonic_step = group_f_base / group_n_base as f64;
    let harmonic_half_span = harmonic_step * 0.5;

    let mut nodes = Vec::with_capacity(divs.len());
    for (i, &d) in divs.iter().enumerate() {
        let center_ratio = 2f64.powf(d as f64 / equal_divisions as f64);
        let base_f = group_f_base * center_ratio;
        if base_f > MAX_FREQ_HZ {
            continue;
        }

        let lower_ratio = 2f64.powf(boundaries[i] / equal_divisions as f64);
        let upper_ratio = 2f64.powf(boundaries[i + 1] / equal_divisions as f64);

        let mut start_f = group_f_base * lower_ratio;
        let mut end_f = group_f_base * upper_ratio;

        // Harmonic cap intersection
        start_f = start_f.max((base_f - harmonic_half_span).max(MIN_FREQ_HZ));
        end_f = end_f.min((base_f + harmonic_half_span).min(MAX_FREQ_HZ));

        if end_f <= start_f {
            // fallback micro-span
            let eps = base_f * 0.001;
            let s = (base_f - eps).max(MIN_FREQ_HZ);
            let e = (base_f + eps).min(MAX_FREQ_HZ);
            if e <= s {
                continue;
            }
            start_f = s;
            end_f = e;
        }

        nodes.push(NodeConfig {
            base_frequency: base_f,
            phase: 0.0,
            band_range: start_f..end_f,
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
fn assign_node_phases(nodes: &mut [NodeConfig], group_index: usize, channel: GroupChanel) {
    if nodes.is_empty() {
        return;
    }
    let group_offset = (group_index as f64) * std::f64::consts::PI / 3.0;
    let channel_offset = match channel {
        GroupChanel::Left => 0.0,
        GroupChanel::Right => std::f64::consts::FRAC_PI_4,
    };
    let n = nodes.len() as f64;
    for (k, node) in nodes.iter_mut().enumerate() {
        let spread = 2.0 * std::f64::consts::PI * (k as f64) / n;
        node.phase = group_offset + spread + channel_offset;
    }
}

/// Map a group's base frequency to an a_coef ∈ [0,1], higher pitch => higher coef.
///
/// Uses logarithmic normalization over the "soft" recommended band. Values
/// below soft min clamp to 0, above soft max clamp to 1. A floor of 0.2 keeps
/// low groups expressive.
fn a_coef_from_frequency(f: f64) -> f32 {
    let min = SOFT_MIN_FREQ_HZ.max(1.0);
    let max = SOFT_MAX_FREQ_HZ;
    let ln_min = min.ln();
    let ln_max = max.ln();
    let norm = if ln_max > ln_min {
        ((f.ln() - ln_min) / (ln_max - ln_min)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (0.2 + 0.8 * norm) as f32
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

            let (group_f_base, group_n_base) = g_channel.compute_fundamentals(l, n);

            let f_base = group_f_base * 2usize.pow(g_x as u32) as f64;

            let mut nodes = build_group_nodes(f_base, group_n_base, scale, equal_divisions);

            // 1) Phase spreading
            assign_node_phases(&mut nodes, g_x, g_channel);

            // 2) a_coef increases with pitch
            let a_coef = a_coef_from_frequency(f_base);

            groups.push(GroupConfig {
                channel: g_channel,
                nodes,
                a_coef,
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
            GroupChanel::Left.nth_channel_from_first(0),
            GroupChanel::Left
        );
        assert_eq!(
            GroupChanel::Left.nth_channel_from_first(1),
            GroupChanel::Right
        );
        assert_eq!(
            GroupChanel::Right.nth_channel_from_first(0),
            GroupChanel::Right
        );
        assert_eq!(
            GroupChanel::Right.nth_channel_from_first(1),
            GroupChanel::Left
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
            band_range: super::SOFT_MIN_FREQ_HZ..super::SOFT_MAX_FREQ_HZ,
        };
        assert!(valid_node.validate(0).is_ok());
        assert!(NodeConfig {
            base_frequency: super::MIN_FREQ_HZ - 1.0,
            phase: 0.0,
            band_range: super::MIN_FREQ_HZ..super::MAX_FREQ_HZ,
        }
        .validate(0)
        .is_err());
    }

    #[test]
    fn test_group_config_validity() {
        let node = NodeConfig {
            base_frequency: (super::SOFT_MIN_FREQ_HZ + super::SOFT_MAX_FREQ_HZ) / 2.0,
            phase: 0.0,
            band_range: super::SOFT_MIN_FREQ_HZ..super::SOFT_MAX_FREQ_HZ,
        };
        let group = GroupConfig {
            channel: GroupChanel::Left,
            nodes: vec![node],
            a_coef: 0.5,
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
    fn test_a_coef_non_decreasing_with_frequency() {
        let layout = layout_test_cases().next().expect("at least one layout");
        let config = Config::try_from(layout).expect("layout should yield a valid config");

        // Build a vector of (min_group_frequency, a_coef)
        // Use min base frequency of each group's nodes as representative
        let mut pairs: Vec<(f64, f32)> = config
            .0
            .iter()
            .map(|g| {
                let min_f = g
                    .nodes
                    .iter()
                    .map(|n| n.base_frequency)
                    .fold(f64::INFINITY, f64::min);
                (min_f, g.a_coef)
            })
            .collect();

        // Sort by frequency
        pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        // Verify non-decreasing a_coef
        for window in pairs.windows(2) {
            let prev = window[0];
            let next = window[1];
            assert!(
                next.1 + 1e-6 >= prev.1,
                "a_coef decreased: {} -> {} (freq {} -> {})",
                prev.1,
                next.1,
                prev.0,
                next.0
            );
        }
    }

    #[test]
    fn test_config_max_volume_check() {
        // Construct a config that should violate the max node budget:
        // MAX_DBS is treated as the hard cap on total simultaneous nodes.
        let excessive_nodes = MAX_DBS + 1;
        let dummy_node = NodeConfig {
            base_frequency: (SOFT_MIN_FREQ_HZ + SOFT_MAX_FREQ_HZ) / 2.0,
            phase: 0.0,
            band_range: SOFT_MIN_FREQ_HZ..SOFT_MAX_FREQ_HZ,
        };
        let group = GroupConfig {
            channel: GroupChanel::Left,
            nodes: vec![dummy_node.clone(); excessive_nodes],
            a_coef: 0.5,
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
            channel: GroupChanel::Left,
            nodes: vec![dummy_node; ok_nodes],
            a_coef: 0.5,
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
