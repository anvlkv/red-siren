use std::{num::NonZeroU16, ops::RemAssign, sync::Arc};

use fundsp::prelude::*;
use parking_lot::RwLock;

use crate::excitor::{ExcitementData, ExcitementSnapshot};

const METRO_TEMPO_ID: u64 = crate::util::hash_str(concat!(module_path!(), "::MetroTempo"));

#[derive(Clone)]
pub struct MetroTempo<
    S: Real
        + Float
        + ordered_float::FloatCore
        + ordered_float::Float
        + RemAssign
        + PartialOrd
        + 'static,
> {
    beat_rate_per_minute: NonZeroU16,
    sample_rate_per_second: S,
    ticks_to_next_bpm_change: usize,
    bpm_tables: Arc<RwLock<Vec<Vec<usize>>>>,
    weight_tables: ExcitementSnapshot<S>,
}

impl<
        S: Real
            + Float
            + ordered_float::FloatCore
            + ordered_float::Float
            + RemAssign
            + PartialOrd
            + 'static,
    > MetroTempo<S>
{
    pub fn new(
        bpm_tables: Arc<RwLock<Vec<Vec<usize>>>>,
        weight_tables: ExcitementSnapshot<S>,
    ) -> Self {
        let beat_rate_per_minute = {
            let bpm_tables = bpm_tables.read();
            let weight_tables = weight_tables.read();

            Self::compute_weighted_bpm(&bpm_tables, &weight_tables, NonZeroU16::new(120).unwrap())
        };

        let ticks_to_next_bpm_change =
            Self::compute_ticks_per_beat(beat_rate_per_minute, convert(DEFAULT_SR), 0, 0);

        Self {
            beat_rate_per_minute: beat_rate_per_minute,
            sample_rate_per_second: convert(DEFAULT_SR),
            ticks_to_next_bpm_change,
            bpm_tables,
            weight_tables,
        }
    }

    /// Computes the number of ticks per beat based on the given BPM and sample rate.
    fn compute_ticks_per_beat(
        bpm: NonZeroU16,
        sample_rate: S,
        ticks_to_next_bpm_change: usize,
        delta_bpm: isize,
    ) -> usize {
        let bpm_f64 = bpm.get() as f64;
        let ticks_per_beat: S = sample_rate / convert(bpm_f64 / 60.0);
        let next_period_ticks = fundsp::Num::round(ticks_per_beat).to_usize().unwrap_or(1);

        // Preserve phase continuity after tempo changes by rescaling remaining ticks from the
        // previous BPM period to the new one.
        if ticks_to_next_bpm_change > 0 && delta_bpm != 0 {
            let previous_bpm = bpm.get() as isize - delta_bpm;
            if previous_bpm > 0 {
                let phase_scaled =
                    (ticks_to_next_bpm_change as f64) * (previous_bpm as f64 / bpm.get() as f64);
                let phase_scaled_s: S = convert(phase_scaled);
                let phase_ticks = fundsp::Num::round(phase_scaled_s).to_usize().unwrap_or(1);
                return std::cmp::max(phase_ticks, 1usize);
            }
        }

        std::cmp::max(next_period_ticks, 1usize)
    }

    /// Computes the weighted BPM based on the provided BPM tables and excitement data.
    ///
    /// Parameters:
    /// - `bpm_tables`: A vector of vectors containing BPM values of each excitement source arranged in bands. By analogy with [common::instrument::BandConfig], [common::instrument::NodeConfig].
    /// - `weight_tables`: A vector of `ExcitementData` containing the current excitement values for each source, arranged in the same order as `bpm_tables`.
    fn compute_weighted_bpm(
        bpm_tables: &Vec<Vec<usize>>,
        weight_tables: &Vec<ExcitementData<S>>,
        previous_bpm: NonZeroU16,
    ) -> NonZeroU16 {
        bpm_tables
            .iter()
            .enumerate()
            .flat_map(|(band_index, band)| {
                band.iter()
                    .enumerate()
                    .map(move |(node_index, node)| (band_index, node_index, node))
            })
            .zip(weight_tables.iter())
            .fold(WeightedBpm::<S>::default(), |mut acc, data| {
                let ((band_index, node_index, bpm_value), excitement_data) = data;

                acc.add(band_index, node_index, *bpm_value, excitement_data);

                acc
            })
            .compute(previous_bpm)
    }
}

struct WeightedBpm<
    S: Real
        + Float
        + ordered_float::FloatCore
        + ordered_float::Float
        + RemAssign
        + PartialOrd
        + 'static,
> {
    len_bands: usize,
    len_total: usize,
    xct_band_sum: Vec<ExcitementData<S>>,
    xct_total_sum: ExcitementData<S>,
    xct_max: (usize, usize, ExcitementData<S>, usize), // (band_index, node_index, excitement_value, bpm_value)
    bpm_band_sum: Vec<S>,
    bpm_total_sum: S,
    /// Sum of `bpm_value * re` across all nodes; numerator for the re-weighted BPM centroid.
    bpm_re_total_sum: S,
    /// Sum of `re` (energy strength) across all nodes; denominator for the re-weighted BPM centroid.
    re_total_sum: S,
    /// Sum of `im` (spectral offset) across all nodes; used to derive global agitation.
    im_total_sum: S,
}

impl<
        S: Real
            + Float
            + ordered_float::FloatCore
            + ordered_float::Float
            + RemAssign
            + PartialOrd
            + 'static,
    > WeightedBpm<S>
{
    fn add(
        &mut self,
        band_index: usize,
        node_index: usize,
        bpm_value: usize,
        excitement_value: &ExcitementData<S>,
    ) {
        if band_index >= self.len_bands {
            self.len_bands = band_index + 1;
            self.xct_band_sum
                .resize(self.len_bands, ExcitementData::default());
            self.bpm_band_sum.resize(self.len_bands, convert(0.0));
        }

        self.xct_band_sum[band_index] += *excitement_value;
        self.bpm_band_sum[band_index] += convert(bpm_value as f64);

        self.xct_total_sum += *excitement_value;
        self.bpm_total_sum += convert(bpm_value as f64);

        // Accumulate re/im channel sums separately for the real-imag role split in compute.
        let re = excitement_value.re.0;
        let im = excitement_value.im.0;
        let bpm_s: S = convert(bpm_value as f64);
        self.bpm_re_total_sum += bpm_s * re;
        self.re_total_sum += re;
        self.im_total_sum += im;

        if (self.xct_max.2.re, self.xct_max.2.im) < (excitement_value.re, excitement_value.im) {
            self.xct_max = (band_index, node_index, *excitement_value, bpm_value);
        }

        self.len_total += 1;
    }

    fn compute(self, previous_bpm: NonZeroU16) -> NonZeroU16 {
        let Self {
            len_bands: _len_bands,
            len_total,
            xct_band_sum: _xct_band_sum,
            xct_total_sum: _xct_total_sum,
            bpm_band_sum: _bpm_band_sum,
            bpm_re_total_sum,
            re_total_sum,
            im_total_sum,
            xct_max: (_max_band_index, _max_node_index, _max_excitement, max_node_bpm),
            bpm_total_sum,
        } = self;

        if len_total == 0 {
            // No data at all: fall back to a sensible default tempo.
            return NonZeroU16::new(120).unwrap();
        }

        // --- Context averages (available for future strategy variants) ---
        // Unweighted mean BPM across all nodes; used as fallback and for dynamic range.
        let bpm_total_avg = bpm_total_sum / convert(len_total as f64);

        // ExcitementData channel semantics (confirmed from excitor.rs):
        //   re = energy / resonance strength  →  how strongly a node is being excited
        //   im = spectral offset from band center  →  0 = settled/on-target, 1 = fringe/detuned
        //
        // Design intent:
        //   The `re` channel drives a *strength-weighted BPM centroid*: nodes that vibrate harder
        //   pull tempo toward their assigned BPM.
        //
        //   The `im` channel drives *agitation*: when activity clusters near the edges of the
        //   sensor band (tonally searching/unstable), tempo is pulled toward the dominant node's
        //   BPM rather than the crowd average, adding expressive momentum.

        // Step 1 — Re-weighted BPM centroid.
        // Each node's BPM contribution is proportional to its real excitement strength, so
        // louder/stronger nodes shape the tempo more than quiet ones.
        let eps: S = convert(1e-6_f64);
        let bpm_re: S = if re_total_sum > eps {
            bpm_re_total_sum / re_total_sum
        } else {
            // All nodes are silent; fall back to the unweighted average.
            bpm_total_avg
        };

        // Step 2 — Global agitation scalar from mean imaginary magnitude.
        // im → 0: excitation is centered, texture is settled  → low agitation
        // im → 1: excitation is at spectral fringes, texture is searching → high agitation
        let agitation_raw: S = im_total_sum / convert(len_total as f64);
        let agitation: S = if agitation_raw < convert(0.0_f64) {
            convert(0.0_f64)
        } else if agitation_raw > convert(1.0_f64) {
            convert(1.0_f64)
        } else {
            agitation_raw
        };

        // Step 3 — Agitation-driven pull toward the dominant node's BPM.
        // When agitation is high the texture is tonally unstable; we interpret that as energy
        // building toward the loudest node's groove, so tempo is nudged in that direction.
        // k controls the maximum pull strength; 0.45 means agitation can shift tempo by up to
        // 45 % of the gap between the centroid and the dominant node.
        let k: S = convert(0.45_f64);
        let max_bpm_s: S = convert(max_node_bpm as f64);
        let bpm_raw: S = bpm_re + agitation * (max_bpm_s - bpm_re) * k;

        // Step 4 — Saturate into a dynamic range derived from the actual BPM table contents.
        // The range expands/contracts around the unweighted mean, clamped to playable extremes.
        // This avoids hard-coded min/max and respects the instrument's BPM vocabulary.
        let lo_raw: S = bpm_total_avg * convert(0.5_f64);
        let lo: S = if lo_raw < convert(40.0_f64) {
            convert(40.0_f64)
        } else {
            lo_raw
        };
        let hi_raw: S = bpm_total_avg * convert(2.0_f64);
        let hi: S = if hi_raw > convert(280.0_f64) {
            convert(280.0_f64)
        } else {
            hi_raw
        };
        let bpm_clamped: S = if bpm_raw < lo {
            lo
        } else if bpm_raw > hi {
            hi
        } else {
            bpm_raw
        };

        // Step 5 — Previous-BPM continuity.
        // Smooth toward the newly computed target to reduce jitter between updates.
        // High agitation reduces inertia so tempo can react faster when energy is tense.
        let previous_bpm_s: S = convert(previous_bpm.get() as f64);
        let inertia_base: S = convert(0.65_f64);
        let inertia_span: S = convert(0.45_f64);
        let one: S = convert(1.0_f64);
        let inertia: S = inertia_base - agitation * inertia_span;
        let bpm_smoothed: S = previous_bpm_s * inertia + bpm_clamped * (one - inertia);

        // Round to the nearest integer BPM and wrap in NonZeroU16.
        let bpm_int = std::cmp::min(
            fundsp::Num::round(bpm_smoothed).to_usize().unwrap_or(120),
            u16::MAX as usize,
        ) as u16;
        NonZeroU16::new(bpm_int).unwrap_or_else(|| NonZeroU16::new(120).unwrap())
    }
}

impl<
        S: Real
            + Float
            + ordered_float::FloatCore
            + ordered_float::Float
            + RemAssign
            + PartialOrd
            + 'static,
    > Default for WeightedBpm<S>
{
    fn default() -> Self {
        Self {
            len_bands: 0,
            len_total: 0,
            xct_band_sum: Vec::new(),
            xct_total_sum: ExcitementData::default(),
            bpm_band_sum: Vec::new(),
            bpm_total_sum: convert(0_f64),
            bpm_re_total_sum: convert(0_f64),
            re_total_sum: convert(0_f64),
            im_total_sum: convert(0_f64),
            xct_max: (0, 0, ExcitementData::default(), 0),
        }
    }
}

impl<
        S: Real
            + Float
            + ordered_float::FloatCore
            + ordered_float::Float
            + RemAssign
            + PartialOrd
            + 'static,
    > AudioNode for MetroTempo<S>
{
    const ID: u64 = METRO_TEMPO_ID;

    type Inputs = U0;

    type Outputs = U1;

    fn tick(&mut self, _input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        if self
            .ticks_to_next_bpm_change
            .checked_sub(1)
            .is_some_and(|v| v > 0)
        {
            self.ticks_to_next_bpm_change -= 1;
        } else {
            let previous_bpm = self.beat_rate_per_minute;
            self.beat_rate_per_minute = {
                let bpm_tables = self.bpm_tables.read();
                let weight_tables = self.weight_tables.read();
                Self::compute_weighted_bpm(&bpm_tables, &weight_tables, previous_bpm)
            };
            let delta_bpm = self.beat_rate_per_minute.get() as isize - previous_bpm.get() as isize;
            self.ticks_to_next_bpm_change = Self::compute_ticks_per_beat(
                self.beat_rate_per_minute,
                self.sample_rate_per_second,
                self.ticks_to_next_bpm_change,
                delta_bpm,
            )
            .saturating_sub(1);
        }

        [convert(self.beat_rate_per_minute.get() as f32)].into()
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate_per_second = convert(sample_rate);
        self.ticks_to_next_bpm_change = Self::compute_ticks_per_beat(
            self.beat_rate_per_minute,
            self.sample_rate_per_second,
            0,
            0,
        );
    }
}

pub fn create_metro_tempo<
    S: Real
        + Float
        + ordered_float::FloatCore
        + ordered_float::Float
        + RemAssign
        + PartialOrd
        + 'static,
>(
    bpm_tables: Arc<RwLock<Vec<Vec<usize>>>>,
    weight_tables: ExcitementSnapshot<S>,
) -> An<MetroTempo<S>> {
    An(MetroTempo::new(bpm_tables, weight_tables))
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta_fun::prelude::*;
    use num_complex::Complex;
    use ordered_float::OrderedFloat;
    use std::sync::Arc;

    use parking_lot::RwLock;

    fn snapshot_config(num_samples: usize) -> SnapshotConfig {
        SnapshotConfigBuilder::default()
            .sample_rate(100.0)
            .num_samples(num_samples)
            .chart_layout(Layout::CombinedPerChannelType)
            .build()
            .unwrap()
    }

    fn x(re: f32, im: f32) -> ExcitementData<f32> {
        Complex::new(OrderedFloat(re), OrderedFloat(im))
    }

    fn bpm_tables() -> Vec<Vec<usize>> {
        vec![vec![60, 180]]
    }

    fn calm_weights() -> Vec<ExcitementData<f32>> {
        vec![x(1.0, 0.0), x(1.0, 0.0)]
    }

    fn agitated_weights() -> Vec<ExcitementData<f32>> {
        vec![x(0.25, 0.0), x(1.75, 1.0)]
    }

    fn metro_tempo_for_snapshot(
        bpm_tables: Vec<Vec<usize>>,
        weight_tables: Vec<ExcitementData<f32>>,
    ) -> MetroTempo<f32> {
        MetroTempo::new(
            Arc::new(RwLock::new(bpm_tables)),
            Arc::new(RwLock::new(weight_tables)),
        )
    }

    #[test]
    fn compute_ticks_per_beat_rescales_remaining_ticks_on_bpm_increase() {
        let ticks =
            MetroTempo::<f32>::compute_ticks_per_beat(NonZeroU16::new(120).unwrap(), 100.0, 30, 60);

        // Previous BPM was 60, current is 120, so remaining phase should halve.
        assert_eq!(ticks, 15);
    }

    #[test]
    fn compute_ticks_per_beat_rescales_remaining_ticks_on_bpm_decrease() {
        let ticks =
            MetroTempo::<f32>::compute_ticks_per_beat(NonZeroU16::new(60).unwrap(), 100.0, 30, -60);

        // Previous BPM was 120, current is 60, so remaining phase should double.
        assert_eq!(ticks, 60);
    }

    #[test]
    fn compute_ticks_per_beat_without_delta_returns_period_ticks() {
        let ticks =
            MetroTempo::<f32>::compute_ticks_per_beat(NonZeroU16::new(60).unwrap(), 100.0, 0, 0);

        assert_eq!(ticks, 100);
    }

    #[test]
    fn compute_weighted_bpm_blends_with_previous_bpm() {
        let tables = bpm_tables();
        let weights = calm_weights();

        let low_prev = MetroTempo::<f32>::compute_weighted_bpm(
            &tables,
            &weights,
            NonZeroU16::new(60).unwrap(),
        );
        let high_prev = MetroTempo::<f32>::compute_weighted_bpm(
            &tables,
            &weights,
            NonZeroU16::new(180).unwrap(),
        );

        // With calm agitation, target is 120 and smoothing should retain memory.
        assert_eq!(low_prev.get(), 81);
        assert_eq!(high_prev.get(), 159);
    }

    #[test]
    fn metro_tempo_audio_snapshot_calm_weights() {
        let metro = An(metro_tempo_for_snapshot(bpm_tables(), calm_weights()));

        assert_audio_unit_snapshot!(
            "metro_tempo_calm_weights",
            metro,
            InputSource::None,
            snapshot_config(320)
        );
    }

    #[test]
    fn metro_tempo_audio_snapshot_agitated_weights() {
        let metro = An(metro_tempo_for_snapshot(bpm_tables(), agitated_weights()));

        assert_audio_unit_snapshot!(
            "metro_tempo_agitated_weights",
            metro,
            InputSource::None,
            snapshot_config(320)
        );
    }
}
