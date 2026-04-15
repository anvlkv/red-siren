use fundsp::{
    buffer::{BufferMut, BufferRef},
    net::Net,
    prelude::*,
    signal::SignalFrame,
};

use crate::util::hash_str;

const GRID_ID: u64 = hash_str(concat!(module_path!(), "::Grid"));

/// A metro grid that emits a single `1.0` pulse on every beat boundary and
/// `0.0` on all other samples.
///
/// BPM is read from a [`Shared`] cell so it can be changed at any time without
/// rebuilding the network.
///
/// - **Inputs:** 0
/// - **Outputs:** 1  (`1.0` on beat, `0.0` otherwise)
#[derive(Clone)]
pub struct Grid {
    sample_rate: f64,
    /// Cached number of samples between beats at the current BPM.
    beat_samples: u64,
    /// Sample counter within the current beat interval.
    counter: u64,
    /// Runtime-adjustable beats-per-minute.
    bpm_shared: Shared,
}

impl Grid {
    /// Create a new `Grid`.
    ///
    /// `bpm` is a [`Shared`] cell so callers can change the tempo live.
    /// The first output sample will be a beat pulse (counter starts at 0).
    pub fn new(sample_rate: f64, bpm: &Shared) -> Self {
        let bpm_val = bpm.value().max(1.0) as f64;
        let beat_samples = ((60.0 / bpm_val) * sample_rate).round() as u64;
        Self {
            sample_rate,
            beat_samples: Ord::max(beat_samples, 1),
            counter: 0,
            bpm_shared: bpm.clone(),
        }
    }

    /// Re-read the BPM shared value and update `beat_samples`.
    ///
    /// Called at the start of every tick/block so tempo changes take effect
    /// without any extra mechanism.
    fn refresh_beat_samples(&mut self) {
        let bpm = self.bpm_shared.value().max(1.0) as f64;
        self.beat_samples = ((60.0 / bpm) * self.sample_rate).round() as u64;
        self.beat_samples = Ord::max(self.beat_samples, 1);
    }
}

impl AudioUnit for Grid {
    fn inputs(&self) -> usize {
        0
    }

    fn outputs(&self) -> usize {
        1
    }

    /// Emit `1.0` when `counter == 0` (beat boundary), `0.0` otherwise.
    fn tick(&mut self, _input: &[f32], output: &mut [f32]) {
        self.refresh_beat_samples();

        output[0] = if self.counter == 0 { 1.0 } else { 0.0 };

        self.counter += 1;
        if self.counter >= self.beat_samples {
            self.counter = 0;
        }
    }

    fn process(&mut self, size: usize, _input: &BufferRef, output: &mut BufferMut) {
        self.refresh_beat_samples();

        // `size` is a scalar sample count; `at_mut` takes a SIMD frame index.
        // Each SIMD frame holds 8 scalar samples, so we advance the beat counter
        // for all 8 lanes individually before writing the packed vector.
        const SIMD: usize = 8;
        let simd_frames = size / SIMD;
        for s in 0..simd_frames {
            let mut arr = [0.0_f32; 8];
            for k in 0..SIMD {
                arr[k] = if self.counter == 0 { 1.0 } else { 0.0 };
                self.counter += 1;
                if self.counter >= self.beat_samples {
                    self.counter = 0;
                }
            }
            *output.at_mut(0, s) = wide::f32x8::from(arr);
        }
        // Handle any remaining scalar samples (when size is not divisible by SIMD).
        for j in (simd_frames * SIMD)..size {
            let pulse = if self.counter == 0 { 1.0 } else { 0.0 };
            self.counter += 1;
            if self.counter >= self.beat_samples {
                self.counter = 0;
            }
            output.set_f32(0, j, pulse);
        }
    }

    fn set_sample_rate(&mut self, rate: f64) {
        self.sample_rate = rate;
        self.refresh_beat_samples();
        // Keep counter in range after a sample-rate change.
        if self.counter >= self.beat_samples {
            self.counter = 0;
        }
    }

    fn reset(&mut self) {
        self.counter = 0;
        self.refresh_beat_samples();
    }

    fn allocate(&mut self) {}

    fn route(&mut self, _input: &SignalFrame, _frequency: f64) -> SignalFrame {
        SignalFrame::new(1)
    }

    fn get_id(&self) -> u64 {
        GRID_ID
    }

    fn footprint(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

/// Push a [`Grid`] node into `net` and return its [`NodeId`].
///
/// The caller keeps the [`Shared`] BPM handle and can change the tempo at any
/// time by calling `bpm.set_value(new_bpm)`.
pub fn mount_metro_an(net: &mut Net, bpm: &Shared) -> NodeId {
    let sample_rate = DEFAULT_SR;
    net.push(Box::new(Grid::new(sample_rate, bpm)))
}

/// Per-node position and length descriptor within the metro grid.
///
/// All four numerator/denominator values are [`Shared`] so the UI can adjust
/// them live without rebuilding the network.
///
/// Semantics:
/// - `start = start_num / start_den` — grid position of the first tick
///   (e.g. `3/4` means the node starts on beat 3 of a 4/4 bar).
/// - `duration = duration_num / duration_den` — segment length in grid units
///   (e.g. `1/1` = one full bar, `1/4` = one beat).
#[derive(Clone)]
pub struct Cadastre {
    pub start_num: Shared,
    pub start_den: Shared,
    pub duration_num: Shared,
    pub duration_den: Shared,
}

impl Cadastre {
    /// Create a `Cadastre` starting on beat 1 with a duration of one full bar (`1/1`).
    pub fn new() -> Self {
        Self {
            start_num: shared(1.0),
            start_den: shared(1.0),
            duration_num: shared(1.0),
            duration_den: shared(1.0),
        }
    }

    /// Read the current start position as a floating-point ratio.
    pub fn start(&self) -> f32 {
        let den = self.start_den.value();
        if den == 0.0 {
            return 0.0;
        }
        self.start_num.value() / den
    }

    /// Read the current duration as a floating-point ratio.
    pub fn duration(&self) -> f32 {
        let den = self.duration_den.value();
        if den == 0.0 {
            return 0.0;
        }
        self.duration_num.value() / den
    }
}

impl Default for Cadastre {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f64 = 48_000.0;
    const BPM: f32 = 120.0;

    fn make_grid() -> Grid {
        let bpm = shared(BPM);
        Grid::new(SR, &bpm)
    }

    #[test]
    fn first_sample_is_beat() {
        let mut grid = make_grid();
        let mut out = [0.0_f32; 1];
        grid.tick(&[], &mut out);
        assert_eq!(out[0], 1.0, "first tick should be a beat pulse");
    }

    #[test]
    fn second_sample_is_silent() {
        let mut grid = make_grid();
        let mut out = [0.0_f32; 1];
        grid.tick(&[], &mut out); // beat
        grid.tick(&[], &mut out); // not a beat
        assert_eq!(out[0], 0.0, "second tick should be silent");
    }

    #[test]
    fn beat_period_matches_bpm() {
        let mut grid = make_grid();
        // At 120 BPM, 48 kHz → one beat every 24 000 samples.
        let expected = ((60.0 / BPM as f64) * SR).round() as usize;

        let mut out = [0.0_f32; 1];
        grid.tick(&[], &mut out); // beat at 0
        let mut count = 1usize;
        loop {
            grid.tick(&[], &mut out);
            count += 1;
            if out[0] == 1.0 {
                break;
            }
            assert!(count <= expected + 1, "beat overdue at sample {count}");
        }
        assert_eq!(count - 1, expected, "beat interval mismatch");
    }

    #[test]
    fn reset_restarts_counter() {
        let mut grid = make_grid();
        let mut out = [0.0_f32; 1];
        // Advance a few samples.
        for _ in 0..5 {
            grid.tick(&[], &mut out);
        }
        grid.reset();
        grid.tick(&[], &mut out);
        assert_eq!(out[0], 1.0, "beat should fire immediately after reset");
    }

    #[test]
    fn live_bpm_change_takes_effect() {
        let bpm = shared(120.0_f32);
        let mut grid = Grid::new(SR, &bpm);
        let mut out = [0.0_f32; 1];

        // Consume the initial beat.
        grid.tick(&[], &mut out);

        // Change BPM to 240 — half the beat interval.
        bpm.set_value(240.0);

        let expected_new = ((60.0 / 240.0_f64) * SR).round() as usize;
        let mut count = 1usize;
        loop {
            grid.tick(&[], &mut out);
            count += 1;
            if out[0] == 1.0 {
                break;
            }
            assert!(count <= expected_new + 2, "beat overdue after BPM change");
        }
        // Allow ±1 for rounding at the boundary where the change happened.
        assert!(
            count <= expected_new + 1,
            "beat interval should match new BPM, got {count}, expected ~{expected_new}"
        );
    }

    #[test]
    fn mount_metro_an_returns_valid_id() {
        let mut net = Net::new(0, 1);
        let bpm = shared(120.0_f32);
        let id = mount_metro_an(&mut net, &bpm);
        net.pipe_output(id);
        net.check();
        let _ = id;
    }

    #[test]
    fn cadastre_default_ratios() {
        let c = Cadastre::new();
        assert_eq!(c.start(), 1.0);
        assert_eq!(c.duration(), 1.0);
    }

    #[test]
    fn cadastre_live_change() {
        let c = Cadastre::new();
        c.start_num.set_value(3.0);
        c.start_den.set_value(4.0);
        assert!((c.start() - 0.75).abs() < 1e-6);
    }

    #[test]
    fn cadastre_zero_denominator_returns_zero() {
        let c = Cadastre::new();
        c.duration_den.set_value(0.0);
        assert_eq!(c.duration(), 0.0, "division by zero should yield 0.0");
    }
}
