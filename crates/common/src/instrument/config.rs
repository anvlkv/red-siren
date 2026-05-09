mod band;
mod channel;
mod node;
mod scale;

use serde::{Deserialize, Serialize};

use crate::{error::InstrumentConfigError, NodeKey};

use super::{consts::*, Layout};

pub use band::*;
pub use channel::*;
pub use node::*;
pub use scale::*;

pub const RESONANCE_MIN_HARMONICS: usize = 3;
pub const RESONANCE_MAX_HARMONICS: usize = 12;
pub const RESONANCE_MIN_GAMMA: f32 = 0.08;
pub const RESONANCE_MAX_GAMMA: f32 = 0.35;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResonanceModel {
    pub harmonics: usize,
    pub gamma: f32,
}

impl Default for ResonanceModel {
    fn default() -> Self {
        Self {
            harmonics: 6,
            gamma: 0.18,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
/// Instrument configuration for audio generation
pub struct Config(pub Vec<BandConfig>, pub Scale);

impl Config {
    fn resolve_node_indices(&self, key: &NodeKey) -> Option<(usize, usize)> {
        let band_idx = key.band() as usize;
        let key_idx = key.key() as usize;

        // Native path: NodeKey uses zero-based indexing.
        if self
            .0
            .get(band_idx)
            .and_then(|band| band.nodes.get(key_idx))
            .is_some()
        {
            return Some((band_idx, key_idx));
        }

        // Compatibility fallback for any legacy one-based callers.
        let band_idx = key.band().checked_sub(1)? as usize;
        let key_idx = key.key().checked_sub(1)? as usize;
        self.0
            .get(band_idx)
            .and_then(|band| band.nodes.get(key_idx))
            .map(|_| (band_idx, key_idx))
    }

    /// Lookup a node by its key, returning None if not found.
    pub fn get_node(&self, key: &NodeKey) -> Option<&NodeConfig> {
        self.resolve_node_indices(key)
            .and_then(|(band_idx, key_idx)| {
                self.0
                    .get(band_idx)
                    .and_then(|band| band.nodes.get(key_idx))
            })
    }

    /// Lookup the index of a node by its key, returning None if not found.
    ///
    /// The index is a flat index across all bands.
    pub fn get_node_index(&self, key: &NodeKey) -> Option<usize> {
        self.resolve_node_indices(key).map(|(band_idx, key_idx)| {
            let nodes_per_band = self.0.first().map_or(0, |b| b.nodes.len());
            band_idx * nodes_per_band + key_idx
        })
    }

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
            return Err(InstrumentConfigError::InvalidBands);
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

    /// Validation of all nodes in all bands
    ///
    /// - Resonate in recommended frequencies
    /// - Total volume does not exceed max dB
    fn validate_channels(&self) -> Result<(), InstrumentConfigError> {
        let mut prev: Option<BandChannel> = None;
        for g in &self.0 {
            if prev == Some(g.channel) {
                return Err(InstrumentConfigError::ChannelsConfigurationInvalid);
            }
            prev = Some(g.channel);
        }
        Ok(())
    }

    pub fn band_nth_channel(
        &self,
        channel: BandChannel,
        nth_in_channel: usize,
    ) -> Option<&BandConfig> {
        self.0
            .iter()
            .filter(|g| g.channel == channel)
            .nth(nth_in_channel)
    }

    pub fn channel_of_key(&self, key: &NodeKey) -> Option<BandChannel> {
        self.0
            .iter()
            .find(|g| g.nodes.iter().any(|n| &n.key == key))
            .map(|g| g.channel)
    }

    pub fn num_bands_left(&self) -> usize {
        self.0
            .iter()
            .filter(|g| matches!(g.channel, BandChannel::Left))
            .count()
    }

    pub fn num_bands_right(&self) -> usize {
        self.0
            .iter()
            .filter(|g| matches!(g.channel, BandChannel::Right))
            .count()
    }

    pub fn num_bands(&self) -> usize {
        self.0.len()
    }

    pub fn num_nodes_per_band(&self) -> usize {
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

    pub fn nodes_iter(&self) -> impl Iterator<Item = &NodeConfig> {
        self.0.iter().flat_map(|g| g.nodes.iter())
    }

    pub fn bpm_tables(&self) -> Vec<Vec<usize>> {
        self.0
            .iter()
            .map(|b| b.nodes.iter().map(|n| n.hr_bpm() as usize).collect())
            .collect()
    }

    /// Derive a global resonance model from the generated instrument physics.
    ///
    /// This model is intentionally computed from config data (not user-edited),
    /// so the Excitor's harmonic-likeness response follows layout/instrument changes.
    /// Uses the same physics-driven approach as `TryFrom<Layout>`: normalized frequency
    /// positions, Mersenne-law mass relationships, and weighted aggregation of physical
    /// properties to determine resonance parameters.
    pub fn resonance_model(&self) -> ResonanceModel {
        let mut nodes: Vec<_> = self.0.iter().flat_map(|band| band.nodes.iter()).collect();

        if nodes.len() < 2 {
            return ResonanceModel::default();
        }

        nodes.sort_by(|a, b| a.frequency.partial_cmp(&b.frequency).unwrap());

        let min_f = nodes.first().unwrap().frequency;
        let max_f = nodes.last().unwrap().frequency;
        let ln_f_range = (max_f / min_f).ln();

        // Frequency span in octaves: justified by complexity of the resonance field
        let span_octaves = (max_f / min_f).log2().clamp(0.0, 8.0);

        // HARMONICS: derive from generated instrument structure and resulting span.
        // - `num_nodes_per_band` captures octave divisions (modal density per octave)
        // - `num_bands` captures octave count (harmonic stack depth)
        // - `span_octaves` captures effective realized frequency spread
        let octave_divisions = self.num_nodes_per_band().max(1) as f64;
        let octave_count = self.num_bands().max(1) as f64;
        let harmonics = (2.0 + 0.9 * octave_count + 0.35 * octave_divisions + 0.25 * span_octaves)
            .round()
            .clamp(
                RESONANCE_MIN_HARMONICS as f64,
                RESONANCE_MAX_HARMONICS as f64,
            ) as usize;

        // GAMMA: Derive from physical properties using normalized-position approach
        // (matching the TryFrom<Layout> methodology for consistency).
        // Lower-frequency instruments (heavier resonators per Mersenne law) have broader resonance curves.
        let mut mass_weighted_sum = 0.0;
        for node in &nodes {
            // Normalize frequency position: t=0 (low freq, heavy) → t=1 (high freq, light)
            // Same calculation as TryFrom<Layout>
            let t = if ln_f_range.abs() < f64::EPSILON {
                0.5
            } else {
                ((node.frequency.ln() - min_f.ln()) / ln_f_range).clamp(0.0, 1.0)
            };

            // Weight by node mass, giving more influence to low-frequency (heavier) nodes
            // This captures the physical intuition: heavy resonators have broader damping
            mass_weighted_sum += node.w_kg * (1.0 - t);
        }

        let avg_weighted_mass = mass_weighted_sum / nodes.len() as f64;
        let mass_normalized =
            ((avg_weighted_mass - W_MIN_KG) / (W_MAX_KG - W_MIN_KG)).clamp(0.0, 1.0);

        // Map normalized mass to gamma: heavier instruments → broader coupling bandwidth
        let gamma = RESONANCE_MIN_GAMMA as f64
            + mass_normalized * (RESONANCE_MAX_GAMMA as f64 - RESONANCE_MIN_GAMMA as f64);
        let gamma = (gamma as f32).clamp(RESONANCE_MIN_GAMMA, RESONANCE_MAX_GAMMA);

        ResonanceModel { harmonics, gamma }
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
        let num_bands = key_registry.num_bands() as usize;
        let num_divisions_per_band = key_registry.num_keys_per_band() as usize;
        let scale = layout.scale;

        // computed properties
        let total_steps = key_registry.total_keys();
        let phase_step = 1.0 / total_steps as f64;

        // Pre-compute frequency range across all bands to anchor mass/volume.
        // Mersenne law: w ∝ 1/f², so log(w) is linear in log(f).
        let octave_bases = collect_octave_bases(string_len, num_bands);
        let f_config_min = scale.freq_n(0.0, octave_bases[0], num_divisions_per_band as f64);
        let f_config_max = scale.freq_n(
            (num_divisions_per_band - 1) as f64,
            *octave_bases.last().unwrap_or(&octave_bases[0]),
            num_divisions_per_band as f64,
        );
        let ln_f_range = f_config_max.ln() - f_config_min.ln();

        // track values
        let mut n = 0;
        let mut n_base = 1;

        let mut bands = Vec::new();
        let mut target_f_min = None;
        for g in 0..num_bands {
            let channel = layout.first_band_channel.nth_channel_from_first(g);

            let (octave_f_base, next_n_base) =
                compute_fundamentals(string_len, n_base, target_f_min);

            let mut nodes = vec![];

            for k in 0..num_divisions_per_band {
                let key = key_registry.create_key(g.try_into()?, k.try_into()?)?;

                let frequency =
                    scale.freq_n(k as f64, octave_f_base, num_divisions_per_band as f64);

                let phase = phase_step * n as f64;
                let cents = 1200.0 * (frequency / octave_f_base).log2();

                // t=0: lowest frequency, t=1: highest frequency.
                // Guard against degenerate single-frequency configs.
                let t = if ln_f_range.abs() < f64::EPSILON {
                    0.5
                } else {
                    ((frequency.ln() - f_config_min.ln()) / ln_f_range).clamp(0.0, 1.0)
                };

                // Mass decreases with frequency (high freq = lighter resonator).
                let w_kg = W_MAX_KG * (W_MIN_KG / W_MAX_KG).powf(t);

                // Density rises toward high pitch with a custom smoothstep curve.
                let density_curve = t * t * (3.0 - 2.0 * t);

                // Volume derived from mass and body density: v = m / ρ
                let body_density = BODY_DENSITY_MIN_G_CM3
                    * (BODY_DENSITY_MAX_G_CM3 / BODY_DENSITY_MIN_G_CM3).powf(density_curve);
                let v_cm3 = (w_kg * 1000.0) / body_density;

                // Acoustic resonator length derived from volume (not spatial position)
                let l_mm = L_MM_ACOUSTIC_SCALE * v_cm3.cbrt();

                nodes.push(NodeConfig {
                    key,
                    frequency,
                    l_mm,
                    w_kg,
                    v_cm3,
                    phase,
                    cents,
                });

                n += 1;
            }

            target_f_min = Some(octave_f_base * 2.0);
            n_base = (next_n_base + 1).max(g + 1);

            nodes.sort();

            bands.push(BandConfig { channel, nodes });
        }

        let config = Config(bands, scale);

        // Single-pass validation (recommended + safe)
        // Caller can decide how to surface any error.
        config.validate()?;

        Ok(config)
    }
}

fn fundamental_frequency(n: usize, v: f64, l: f64) -> f64 {
    (n as f64 * v) / (2.0 * l)
}

/// Pre-computes the octave base frequency for each band without building nodes.
/// Mirrors the outer band loop in `TryFrom<Layout> for Config`, tracking the same
/// `n_base` / `target_f_min` state so the returned bases match the real generation.
fn collect_octave_bases(string_len: f64, num_bands: usize) -> Vec<f64> {
    let mut bases = Vec::with_capacity(num_bands);
    let mut n_base: usize = 1;
    let mut target_f_min: Option<f64> = None;
    for g in 0..num_bands {
        let (octave_f_base, next_n_base) = compute_fundamentals(string_len, n_base, target_f_min);
        bases.push(octave_f_base);
        target_f_min = Some(octave_f_base * 2.0);
        n_base = (next_n_base + 1).max(g + 1);
    }
    bases
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

#[cfg(any(test, feature = "test"))]
pub fn representative_layout_configs() -> Vec<Config> {
    let all = config_test_cases()
        .map(|(config, _layout)| config)
        .collect::<Vec<_>>();

    assert!(
        all.len() >= 3,
        "expected at least three layout-derived config test cases"
    );

    vec![
        all[0].clone(),
        all[all.len() / 2].clone(),
        all[all.len() - 1].clone(),
    ]
}

#[cfg(any(test, feature = "test"))]
pub fn layout_single_node(seed: usize) -> NodeConfig {
    let config = representative_layout_configs()
        .into_iter()
        .nth(seed % 3)
        .expect("representative layout config");

    config
        .0
        .iter()
        .flat_map(|band| band.nodes.iter().copied())
        .min_by(|a, b| a.room_size_m3().total_cmp(&b.room_size_m3()))
        .expect("layout config must contain at least one node")
}

#[cfg(any(test, feature = "test"))]
pub fn layout_node_with_key(key: NodeKey, seed: usize) -> NodeConfig {
    let mut node = layout_single_node(seed);
    node.key = key;
    node
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instrument::layout::layout_test_cases;
    use insta::assert_json_snapshot;
    use serde::Serialize;

    #[derive(Debug, Serialize)]
    struct ResonanceModelSnapshotCase {
        layout_space_x: f64,
        layout_space_y: f64,
        layout_scale: String,
        harmonics: usize,
        gamma: f32,
    }

    #[test]
    fn test_config_from_layout_validity() {
        for layout in layout_test_cases() {
            let config = Config::try_from(layout).expect("Config from layout should be valid");
            assert_json_snapshot!(
                format!(
                    "config_{}x{}_{:?}",
                    layout.space.x, layout.space.y, layout.scale
                ),
                config,
                {
                    "[0][].nodes[].frequency" => insta::rounded_redaction(4),
                    "[0][].nodes[].phase" => insta::rounded_redaction(4),
                    "[0][].nodes[].cents" => insta::rounded_redaction(4),
                    "[0][].nodes[].l_mm" => insta::rounded_redaction(4),
                    "[0][].nodes[].w_kg" => insta::rounded_redaction(4),
                    "[0][].nodes[].v_cm3" => insta::rounded_redaction(4)
                }
            );
        }
    }

    #[test]
    fn test_band_channel_nth_channel_from_first() {
        assert_eq!(
            BandChannel::Left.nth_channel_from_first(0),
            BandChannel::Left
        );
        assert_eq!(
            BandChannel::Left.nth_channel_from_first(1),
            BandChannel::Right
        );
        assert_eq!(
            BandChannel::Right.nth_channel_from_first(0),
            BandChannel::Right
        );
        assert_eq!(
            BandChannel::Right.nth_channel_from_first(1),
            BandChannel::Left
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
    fn test_band_config_validity() {
        let node =
            NodeConfig::new_test_node((super::SOFT_MIN_FREQ_HZ + super::SOFT_MAX_FREQ_HZ) / 2.0);
        let band = BandConfig {
            channel: BandChannel::Left,
            nodes: vec![node],
        };
        assert!(band.validate(0).is_ok());
    }

    #[test]
    fn test_config_max_volume_check() {
        // Construct a config that should violate the max node budget:
        // MAX_DBS is treated as the hard cap on total simultaneous nodes.
        let excessive_nodes = MAX_DBS + 1;
        let dummy_node = NodeConfig::new_test_node((SOFT_MIN_FREQ_HZ + SOFT_MAX_FREQ_HZ) / 2.0);
        let band = BandConfig {
            channel: BandChannel::Left,
            nodes: vec![dummy_node; excessive_nodes],
        };
        let cfg_excess = Config(vec![band], Scale::Yo);
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
        let band_ok = BandConfig {
            channel: BandChannel::Left,
            nodes: vec![dummy_node; ok_nodes],
        };
        let cfg_ok = Config(vec![band_ok], Scale::In);
        assert!(
            cfg_ok.max_event_volume_ok(),
            "Node count at limit should pass"
        );
        // Single band: channel alternation not applicable. Uses recommended band; validate() should succeed.
        assert!(
            cfg_ok.validate().is_ok(),
            "Config at volume limit should remain valid"
        );
    }

    #[test]
    fn test_physical_progression_with_frequency() {
        let epsilon = 1e-12;

        for (config, _) in config_test_cases() {
            let mut nodes = config
                .0
                .iter()
                .flat_map(|band| band.nodes.iter().copied())
                .collect::<Vec<_>>();

            nodes.sort_by(|a, b| a.frequency.partial_cmp(&b.frequency).unwrap());

            for pair in nodes.windows(2) {
                let prev = pair[0];
                let next = pair[1];

                assert!(
                    next.w_kg <= prev.w_kg + epsilon,
                    "mass should be non-increasing with frequency: prev={} next={}",
                    prev.w_kg,
                    next.w_kg
                );
                assert!(
                    next.v_cm3 <= prev.v_cm3 + epsilon,
                    "volume should be non-increasing with frequency: prev={} next={}",
                    prev.v_cm3,
                    next.v_cm3
                );
                assert!(
                    next.body_density_g_cm3() + epsilon >= prev.body_density_g_cm3(),
                    "density should be non-decreasing with frequency"
                );
                assert!(
                    next.hr_bpm() >= prev.hr_bpm(),
                    "BPM should be non-decreasing with frequency: prev={} next={}",
                    prev.hr_bpm(),
                    next.hr_bpm()
                );
                assert!(
                    prev.room_size_m3() >= prev.v_m3(),
                    "room size should fit body volume: room={} body={}",
                    prev.room_size_m3(),
                    prev.v_m3()
                );
                assert!(
                    next.room_size_m3() >= next.v_m3(),
                    "room size should fit body volume: room={} body={}",
                    next.room_size_m3(),
                    next.v_m3()
                );
                assert!(
                    next.room_size_m3() <= prev.room_size_m3() + epsilon,
                    "room size should be non-increasing with frequency: prev={} next={}",
                    prev.room_size_m3(),
                    next.room_size_m3()
                );
            }
        }
    }

    #[test]
    fn test_resonance_model_bounds_and_stability() {
        for (config, _) in config_test_cases() {
            let a = config.resonance_model();
            let b = config.resonance_model();

            assert_eq!(a, b, "resonance model should be deterministic");
            assert!(
                (RESONANCE_MIN_HARMONICS..=RESONANCE_MAX_HARMONICS).contains(&a.harmonics),
                "harmonics must stay in configured range"
            );
            assert!(
                (RESONANCE_MIN_GAMMA..=RESONANCE_MAX_GAMMA).contains(&a.gamma),
                "gamma must stay in configured range"
            );
        }
    }

    #[test]
    fn test_resonance_model_snapshot() {
        let cases = config_test_cases()
            .map(|(config, layout)| {
                let model = config.resonance_model();
                ResonanceModelSnapshotCase {
                    layout_space_x: layout.space.x,
                    layout_space_y: layout.space.y,
                    layout_scale: format!("{:?}", layout.scale),
                    harmonics: model.harmonics,
                    gamma: model.gamma,
                }
            })
            .collect::<Vec<_>>();

        assert_json_snapshot!("resonance_model_from_layout_cases", cases, {
            "[].layout_space_x" => insta::rounded_redaction(4),
            "[].layout_space_y" => insta::rounded_redaction(4),
            "[].gamma" => insta::rounded_redaction(6)
        });
    }
}
