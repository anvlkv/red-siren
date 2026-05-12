use serde::{Deserialize, Serialize};

const MIN_AUDIBLE_DURATION_MS: f64 = 10.0;
const K_CYCLES_PER_TICK: f64 = 50_000.0;
const U_BUDGET_PER_THREAD: f64 = 0.05;
/// ---
const BASE_THREAD_UTILIZATION: f64 = 11.0 * U_BUDGET_PER_THREAD;
/// ---
const MAX_THREAD_UTILIZATION: f64 = 17.0 * U_BUDGET_PER_THREAD;
/// Assumes 4 phases per beat (i.e. ADSR) for BPM calculations.
const PHASES_PER_BEAT: f64 = 4.0;

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct GridLimits {
    pub max_bpm: u32,
    pub min_ticks_per_phase: u32,
}

impl GridLimits {
    pub fn new(
        num_threads: usize,
        cpu_frequency_mhz: u64,
        sample_rate_hz: u32,
        buffer_size_samples: u32,
    ) -> Self {
        let threads = num_threads.max(1) as f64;
        let u_eff = (BASE_THREAD_UTILIZATION + U_BUDGET_PER_THREAD * threads.log2())
            .min(MAX_THREAD_UTILIZATION);

        let cpu_cycles_per_second = cpu_frequency_mhz as f64 * 1_000_000.0;
        let tick_rate_cpu_max = (cpu_cycles_per_second * u_eff) / K_CYCLES_PER_TICK;

        let tick_rate_audio = if buffer_size_samples == 0 {
            0.0
        } else {
            sample_rate_hz as f64 / buffer_size_samples as f64
        };

        let tick_rate_eff = tick_rate_audio.min(tick_rate_cpu_max).max(f64::EPSILON);
        let tick_ms_eff = 1_000.0 / tick_rate_eff;

        // A phase should span at least this many ticks to remain audible.
        let min_ticks_per_phase = (MIN_AUDIBLE_DURATION_MS / tick_ms_eff)
            .ceil()
            .max(1.0)
            .min(u32::MAX as f64) as u32;

        let max_bpm = (60_000.0 / (MIN_AUDIBLE_DURATION_MS * PHASES_PER_BEAT))
            .floor()
            .max(1.0)
            .min(u32::MAX as f64) as u32;

        Self {
            max_bpm,
            min_ticks_per_phase,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_positive_limits_for_common_audio_settings() {
        let limits_44k = GridLimits::new(1, 2_000, 44_100, 128);
        let limits_48k = GridLimits::new(1, 2_000, 48_000, 128);
        let limits_96k = GridLimits::new(1, 2_000, 96_000, 256);

        assert!(limits_44k.max_bpm >= 1);
        assert!(limits_44k.min_ticks_per_phase >= 1);

        assert!(limits_48k.max_bpm >= 1);
        assert!(limits_48k.min_ticks_per_phase >= 1);

        assert!(limits_96k.max_bpm >= 1);
        assert!(limits_96k.min_ticks_per_phase >= 1);
    }

    #[test]
    fn is_safe_for_zero_and_low_inputs() {
        let limits = GridLimits::new(0, 0, 0, 0);
        assert!(limits.max_bpm >= 1);
        assert!(limits.min_ticks_per_phase >= 1);
    }

    #[test]
    fn min_ticks_per_phase_respects_audibility_bound() {
        let sample_rate_hz = 48_000;
        let buffer_size_samples = 128;
        let limits = GridLimits::new(1, 2_000, sample_rate_hz, buffer_size_samples);

        let tick_rate_audio = sample_rate_hz as f64 / buffer_size_samples as f64;
        let tick_ms = 1_000.0 / tick_rate_audio;
        let phase_ms = limits.min_ticks_per_phase as f64 * tick_ms;

        assert!(phase_ms >= MIN_AUDIBLE_DURATION_MS);
    }

    #[test]
    fn cpu_budget_non_decreases_with_more_threads_when_cpu_limited() {
        let one_thread = GridLimits::new(1, 1_000, 192_000, 1);
        let eight_threads = GridLimits::new(8, 1_000, 192_000, 1);

        assert!(eight_threads.min_ticks_per_phase >= one_thread.min_ticks_per_phase);
    }
}
