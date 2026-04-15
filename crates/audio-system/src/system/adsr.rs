use dyn_clone::clone_box;
use fundsp::prelude::*;
use num_complex::Complex;

mod generator;
pub use generator::GeneratorNode;
mod phase;
mod scheme;
mod shape;

use phase::Phase;
use scheme::Scheme;

// ── Timing defaults ──────────────────────────────────────────────────────────

const ATTACK_S: f64 = 0.01;
const DECAY_S: f64 = 0.05;
const SUSTAIN_S: f64 = 0.10;
const RELEASE_S: f64 = 0.30;

/// Duration of the single-cycle wavetable rendered from the generator (seconds).
const TABLE_DURATION_S: f64 = 0.05;

const TABLES_PER_OCTAVE: f64 = 12.0;
const DEFAULT_FREQ_HZ: f32 = 440.0;
const EXCITE_THRESHOLD: f32 = 0.01;

/// Fixed inputs before the generator-specific inputs.
///
/// Layout:
/// ```text
/// [0]  metro_tick          (≥ 0.5 = beat pulse)
/// [1]  excite_primary      (real  part of excitement)
/// [2]  excite_secondary    (imaginary part of excitement)
/// [3]  cadastre_start_num
/// [4]  cadastre_start_den
/// [5]  cadastre_duration_num
/// [6]  cadastre_duration_den
/// [7+] generator inputs    (toggle at [7], frequency at [8], …)
/// ```
const NUM_FIXED_INPUTS: usize = 7;

// ── ActivePhase ──────────────────────────────────────────────────────────────

/// One independent ADSR voice inside a [`QueuingAdsr`].
#[derive(Clone)]
struct ActivePhase {
    phase: Phase<f32>,
    /// Current envelope amplitude.
    running_value: f32,
    /// Oscillator position in the wavetable, 0.0 … 1.0.
    wt_pos: f32,
    /// Cached wavetable transposition-table index for efficient lookup.
    table_hint: usize,
}

impl ActivePhase {
    /// Create a new voice starting an Attack toward `target`.
    fn new_attack(target: f32, scheme: &Scheme) -> Self {
        Self {
            phase: Phase::Attack {
                steps: scheme.atack,
                target,
            },
            running_value: 0.0,
            wt_pos: 0.0,
            table_hint: 0,
        }
    }

    /// Advance the envelope by one sample.
    ///
    /// Returns `false` once the voice reaches `Idle` and can be discarded.
    fn tick_envelope(&mut self, scheme: &Scheme, shape_a: f32) -> bool {
        // Phase is Copy — capture a local copy so we can call consuming methods.
        let phase = self.phase;
        if matches!(phase, Phase::Idle) {
            return false;
        }

        let target = phase.target_value(self.running_value);
        let steps = phase.remaining_steps();
        let increment = if steps > 0 {
            (target - self.running_value) / steps as f32
        } else {
            0.0
        };

        match phase.tick() {
            Some(next) => {
                self.phase = next;
                self.running_value += increment;
            }
            None => {
                // This phase's last step was consumed → advance to next stage.
                self.phase = phase.next_phase(scheme, shape_a);
                self.running_value = target;
            }
        }

        !matches!(self.phase, Phase::Idle)
    }
}

// ── QueuingAdsr ──────────────────────────────────────────────────────────────

/// Per-node bandlimited wavetable oscillator with a queuing polyphonic ADSR.
///
/// # Input layout
///
/// `inputs() == NUM_FIXED_INPUTS + num_gen_inputs`
///
/// | idx | meaning |
/// |-----|---------|
/// | 0   | metro tick (≥ 0.5 triggers a new voice on the next beat) |
/// | 1   | excite primary (real) |
/// | 2   | excite secondary (imaginary) |
/// | 3–6 | cadastre rationals (start num/den, duration num/den) |
/// | 7   | toggle (< 0.5 = muted) |
/// | 8   | frequency in Hz |
/// | 9+  | additional generator inputs |
///
/// # Output layout
///
/// `outputs() == 1` — averaged sample across all active voices.
// Manual Clone impl because Box<dyn AudioUnit> is cloned via dyn_clone::clone_box.
impl Clone for QueuingAdsr {
    fn clone(&self) -> Self {
        Self {
            phaser: self.phaser.clone(),
            base_scheme: self.base_scheme,
            sample_rate: self.sample_rate,
            table: self.table.clone(),
            table_len: self.table_len,
            min_pitch: self.min_pitch,
            max_pitch: self.max_pitch,
            tables_per_octave: self.tables_per_octave,
            generator: clone_box(&*self.generator),
            num_gen_inputs: self.num_gen_inputs,
            last_gen_inputs: self.last_gen_inputs.clone(),
            prev_excitement: self.prev_excitement,
        }
    }
}

pub struct QueuingAdsr {
    // ── voices ──
    phaser: Vec<ActivePhase>,
    base_scheme: Scheme,
    sample_rate: f64,

    // ── wavetable ──
    table: Wavetable,
    table_len: usize,
    min_pitch: f64,
    max_pitch: f64,
    tables_per_octave: f64,

    // ── generator ──
    generator: Box<dyn AudioUnit>,
    num_gen_inputs: usize,
    last_gen_inputs: Vec<f32>,

    // ── excitation ──
    prev_excitement: Complex<f32>,
}

impl QueuingAdsr {
    /// Create a new `QueuingAdsr`.
    ///
    /// `generator` must have exactly **1 input** (frequency in Hz) and
    /// **1 output** (audio sample).  The wavetable is rendered from it
    /// immediately at construction time.
    pub fn new(
        sample_rate: f64,
        mut generator: Box<dyn AudioUnit>,
        num_gen_inputs: usize,
        min_pitch: f64,
        max_pitch: f64,
    ) -> Self {
        let base_scheme = Self::make_scheme(sample_rate);
        let table_len = {
            let raw = (TABLE_DURATION_S * sample_rate).ceil() as usize;
            raw.next_power_of_two()
        };
        let last_gen_inputs = vec![0.0_f32; num_gen_inputs];

        let wave = Self::render_wave(&mut *generator, &last_gen_inputs, table_len);
        let table = Wavetable::from_wave(min_pitch, max_pitch, TABLES_PER_OCTAVE, &wave);

        Self {
            phaser: Vec::new(),
            base_scheme,
            sample_rate,
            table,
            table_len,
            min_pitch,
            max_pitch,
            tables_per_octave: TABLES_PER_OCTAVE,
            generator,
            num_gen_inputs,
            last_gen_inputs,
            prev_excitement: Complex::new(0.0, 0.0),
        }
    }

    // ── helpers ─────────────────────────────────────────────────────────────

    fn make_scheme(sample_rate: f64) -> Scheme {
        Scheme {
            atack: (sample_rate * ATTACK_S) as u64,
            decay: (sample_rate * DECAY_S) as u64,
            sustain: (sample_rate * SUSTAIN_S) as u64,
            release: (sample_rate * RELEASE_S) as u64,
            sample_rate: sample_rate as u64,
        }
    }

    /// Render `table_len` samples from `generator` at the frequency encoded in
    /// `gen_inputs`.  The generator is expected to take a single frequency
    /// input; the frequency is extracted as `gen_inputs[1]` (after toggle) or
    /// `gen_inputs[0]` if only one input is present.
    fn render_wave(
        generator: &mut dyn AudioUnit,
        gen_inputs: &[f32],
        table_len: usize,
    ) -> Vec<f32> {
        generator.reset();
        let freq = gen_inputs
            .get(1)
            .or_else(|| gen_inputs.get(0))
            .copied()
            .unwrap_or(DEFAULT_FREQ_HZ)
            .max(1.0);
        let freq_in = [freq];
        (0..table_len)
            .map(|_| {
                let mut out = [0.0_f32];
                generator.tick(&freq_in, &mut out);
                out[0]
            })
            .collect()
    }

    /// Re-render the wavetable from the current `last_gen_inputs`.
    fn rebuild_wavetable(&mut self) {
        let wave = Self::render_wave(&mut *self.generator, &self.last_gen_inputs, self.table_len);
        self.table = Wavetable::from_wave(
            self.min_pitch,
            self.max_pitch,
            self.tables_per_octave,
            &wave,
        );
        // Invalidate cached table hints on existing voices.
        for ap in &mut self.phaser {
            ap.table_hint = 0;
        }
    }

    /// Queue a new Attack voice aimed at `next.re`.
    ///
    /// A voice is only started when `next.re > EXCITE_THRESHOLD`.
    pub fn update(&mut self, next: Complex<f32>) {
        let target = next.re.clamp(0.0, 1.0);
        if target > EXCITE_THRESHOLD {
            self.phaser
                .push(ActivePhase::new_attack(target, &self.base_scheme));
        }
        self.prev_excitement = next;
    }

    // ── scalar tick ─────────────────────────────────────────────────────────

    /// Process one scalar sample.
    ///
    /// Separated from `AudioUnit::tick` so it can be called per-lane inside
    /// `process`.
    fn tick_one(&mut self, input: &[f32], output: &mut [f32]) {
        let metro_tick = input.get(0).copied().unwrap_or(0.0);
        let excite_re = input.get(1).copied().unwrap_or(0.0);
        let excite_im = input.get(2).copied().unwrap_or(0.0);

        // Cadastre: start_num / start_den → shape_a (decay level multiplier).
        let shape_a = {
            let num = input.get(3).copied().unwrap_or(1.0);
            let den = input.get(4).copied().unwrap_or(1.0);
            if den != 0.0 {
                (num / den).clamp(0.0, 1.0)
            } else {
                0.7
            }
        };

        let gen_inputs: &[f32] = if input.len() > NUM_FIXED_INPUTS {
            &input[NUM_FIXED_INPUTS..]
        } else {
            &[]
        };

        // 1. Detect generator-input change → rebuild wavetable.
        let inputs_changed = gen_inputs.len() != self.last_gen_inputs.len()
            || gen_inputs != self.last_gen_inputs.as_slice();
        if inputs_changed {
            if gen_inputs.len() == self.last_gen_inputs.len() {
                self.last_gen_inputs.copy_from_slice(gen_inputs);
            } else {
                self.last_gen_inputs = gen_inputs.to_vec();
            }
            self.rebuild_wavetable();
        }

        // 2. Derive toggle + frequency from generator inputs.
        let toggle = gen_inputs.get(0).copied().unwrap_or(1.0);
        let frequency = gen_inputs
            .get(1)
            .copied()
            .unwrap_or(DEFAULT_FREQ_HZ)
            .max(1.0);

        // 3. Metro beat + excitement → maybe start a new voice.
        if metro_tick >= 0.5 && toggle >= 0.5 && excite_re > EXCITE_THRESHOLD {
            let excitement = Complex::new(excite_re, excite_im);
            self.update(excitement);
        }

        // 4. Advance all active voices, accumulate output.
        let wt_step = frequency as f64 / self.sample_rate;
        let mut sum = 0.0_f32;
        let mut count = 0usize;

        // Split-borrow: table and scheme are borrowed immutably, phaser mutably.
        let table = &self.table;
        let scheme = &self.base_scheme;

        let mut i = 0;
        while i < self.phaser.len() {
            let ap = &mut self.phaser[i];

            // Advance wavetable oscillator.
            ap.wt_pos = (ap.wt_pos as f64 + wt_step).fract() as f32;
            let (sample, new_hint) = table.read(ap.table_hint, frequency, ap.wt_pos);
            ap.table_hint = new_hint;

            // Advance envelope.
            let still_active = ap.tick_envelope(scheme, shape_a);
            let amplitude = ap.running_value;

            if still_active || amplitude.abs() > 1e-6 {
                sum += sample * amplitude;
                count += 1;
            }

            if still_active {
                i += 1;
            } else {
                // Fast removal — order among voices doesn't matter.
                self.phaser.swap_remove(i);
            }
        }

        output[0] = if count > 0 { sum / count as f32 } else { 0.0 };
    }
}

// ── AudioUnit impl ───────────────────────────────────────────────────────────

impl AudioUnit for QueuingAdsr {
    fn inputs(&self) -> usize {
        NUM_FIXED_INPUTS + self.num_gen_inputs
    }

    fn outputs(&self) -> usize {
        1
    }

    fn tick(&mut self, input: &[f32], output: &mut [f32]) {
        self.tick_one(input, output);
    }

    /// Process `size` scalar samples (up to `MAX_BUFFER_SIZE` = 64).
    ///
    /// `BufferRef::at` / `BufferMut::at_mut` expect a **SIMD frame** index (0..size/8),
    /// so the outer loop runs over `size / 8` frames. Any remaining samples (when `size`
    /// is not a multiple of 8) are handled scalar-by-scalar via `at_f32` / `set_f32`.
    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        let n_in = self.inputs();
        let mut in_scalars = vec![0.0_f32; n_in];
        let mut out_scalar = [0.0_f32; 1];
        let simd_frames = size / 8;

        for s in 0..simd_frames {
            let mut out_arr = [0.0_f32; 8];
            for k in 0..8 {
                for ch in 0..n_in {
                    let chunk: wide::f32x8 = input.at(ch, s);
                    in_scalars[ch] = chunk.to_array()[k];
                }
                self.tick_one(&in_scalars, &mut out_scalar);
                out_arr[k] = out_scalar[0];
            }
            *output.at_mut(0, s) = wide::f32x8::from(out_arr);
        }
        // Handle remaining scalar samples when size is not a multiple of 8.
        for j in (simd_frames * 8)..size {
            for ch in 0..n_in {
                in_scalars[ch] = input.at_f32(ch, j);
            }
            self.tick_one(&in_scalars, &mut out_scalar);
            output.set_f32(0, j, out_scalar[0]);
        }
    }

    fn set_sample_rate(&mut self, rate: f64) {
        let old_scheme = self.base_scheme;
        let d = rate / self.sample_rate;
        self.sample_rate = rate;
        self.base_scheme = Self::make_scheme(rate);

        // Rescale step counters on any in-flight voices.
        for ap in &mut self.phaser {
            ap.phase
                .update_sample_rate(d, &self.base_scheme, &old_scheme);
        }

        // Re-render the wavetable at the new sample rate.
        let new_table_len = {
            let raw = (TABLE_DURATION_S * rate).ceil() as usize;
            raw.next_power_of_two()
        };
        if new_table_len != self.table_len {
            self.table_len = new_table_len;
            self.rebuild_wavetable();
        }
    }

    fn reset(&mut self) {
        self.phaser.clear();
        self.prev_excitement = Complex::new(0.0, 0.0);
        self.rebuild_wavetable();
    }

    fn allocate(&mut self) {}

    fn route(&mut self, _input: &SignalFrame, _frequency: f64) -> SignalFrame {
        SignalFrame::new(1)
    }

    fn get_id(&self) -> u64 {
        // Use a fixed compile-time hash — QueuingAdsr has no AudioNode ID.
        crate::util::hash_str("audio_system::system::adsr::QueuingAdsr")
    }

    fn footprint(&self) -> usize {
        std::mem::size_of::<ActivePhase>() * self.phaser.capacity()
            + self.last_gen_inputs.capacity() * std::mem::size_of::<f32>()
            + self.table_len * std::mem::size_of::<f32>()
    }
}

// ── mount helper ─────────────────────────────────────────────────────────────

/// Push a [`QueuingAdsr`] node into `net` and return its [`NodeId`].
///
/// - `generator` must accept **1 input** (frequency Hz) and produce **1 output**.
/// - `num_gen_inputs` is the number of generator-specific inputs that will be
///   connected *after* the 7 fixed inputs (toggle at slot 0, frequency at slot 1).
/// - `min_pitch` / `max_pitch` bound the wavetable's frequency range (Hz).
///
/// The actual sample rate is applied later when `Net::set_sample_rate` is
/// called; the node is constructed at [`DEFAULT_SR`] in the meantime.
pub fn mount_adsr_an(
    net: &mut Net,
    generator: Box<dyn AudioUnit>,
    num_gen_inputs: usize,
    min_pitch: f64,
    max_pitch: f64,
) -> NodeId {
    let adsr = QueuingAdsr::new(DEFAULT_SR, generator, num_gen_inputs, min_pitch, max_pitch);
    net.push(Box::new(adsr))
}

// ── tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f64 = 48_000.0;
    const EPS: f32 = 1.0e-4;

    fn make_adsr() -> QueuingAdsr {
        let generator = Box::new(GeneratorNode::new(SR));
        // num_gen_inputs = 2: [toggle, frequency]
        QueuingAdsr::new(SR, generator, 2, 20.0, 20_000.0)
    }

    /// Build a full input slice of the right length for `make_adsr`.
    fn input(metro: f32, excite_re: f32, excite_im: f32, toggle: f32, freq: f32) -> Vec<f32> {
        vec![
            metro,     // [0] metro
            excite_re, // [1] excite primary
            excite_im, // [2] excite secondary
            1.0,       // [3] cadastre start_num
            1.0,       // [4] cadastre start_den  → shape_a = 1.0
            1.0,       // [5] cadastre dur_num
            1.0,       // [6] cadastre dur_den
            toggle,    // [7] toggle
            freq,      // [8] frequency
        ]
    }

    #[test]
    fn idle_returns_zero() {
        let mut adsr = make_adsr();
        let inp = input(0.0, 0.0, 0.0, 1.0, 440.0);
        let mut out = [0.0_f32];
        adsr.tick(&inp, &mut out);
        assert!(
            out[0].abs() <= EPS,
            "idle tick should be ~0, got {}",
            out[0]
        );
    }

    #[test]
    fn inputs_outputs() {
        let adsr = make_adsr();
        // 7 fixed + 2 gen = 9
        assert_eq!(adsr.inputs(), 9);
        assert_eq!(adsr.outputs(), 1);
    }

    #[test]
    fn metro_beat_with_excitement_starts_voice() {
        let mut adsr = make_adsr();
        let inp = input(1.0, 0.8, 0.0, 1.0, 440.0);
        let mut out = [0.0_f32];

        // Trigger the beat.
        adsr.tick(&inp, &mut out);

        // Run through attack.
        let attack_samples = (SR * ATTACK_S) as usize + 10;
        let mut max_val: f32 = 0.0;
        let silent_inp = input(0.0, 0.0, 0.0, 1.0, 440.0);
        for _ in 0..attack_samples {
            adsr.tick(&silent_inp, &mut out);
            max_val = max_val.max(out[0].abs());
        }
        assert!(
            max_val > EPS,
            "envelope should produce non-zero output during attack, got max {max_val}"
        );
    }

    #[test]
    fn toggle_off_suppresses_new_voice() {
        let mut adsr = make_adsr();
        // Beat + excitement but toggle OFF.
        let inp = input(1.0, 0.8, 0.0, 0.0, 440.0);
        let mut out = [0.0_f32];
        adsr.tick(&inp, &mut out);

        assert_eq!(adsr.phaser.len(), 0, "toggle=0 should not start a voice");
    }

    #[test]
    fn excitement_below_threshold_no_voice() {
        let mut adsr = make_adsr();
        let inp = input(1.0, EXCITE_THRESHOLD * 0.5, 0.0, 1.0, 440.0);
        let mut out = [0.0_f32];
        adsr.tick(&inp, &mut out);

        assert_eq!(
            adsr.phaser.len(),
            0,
            "excitement below threshold should not start a voice"
        );
    }

    #[test]
    fn reset_clears_voices() {
        let mut adsr = make_adsr();
        let inp = input(1.0, 0.8, 0.0, 1.0, 440.0);
        let mut out = [0.0_f32];
        adsr.tick(&inp, &mut out);
        assert!(!adsr.phaser.is_empty());

        adsr.reset();
        assert!(adsr.phaser.is_empty(), "reset should clear all voices");
    }

    #[test]
    fn frequency_change_rebuilds_wavetable() {
        let mut adsr = make_adsr();
        let inp1 = input(0.0, 0.0, 0.0, 1.0, 440.0);
        let inp2 = input(0.0, 0.0, 0.0, 1.0, 880.0);
        let mut out = [0.0_f32];
        adsr.tick(&inp1, &mut out);
        adsr.tick(&inp2, &mut out);
        // No panic and no UB — wavetable rebuilt without error.
    }

    #[test]
    fn multiple_voices_are_averaged() {
        let mut adsr = make_adsr();
        let beat = input(1.0, 0.8, 0.0, 1.0, 440.0);
        let mut out = [0.0_f32];
        // Trigger two separate beats.
        adsr.tick(&beat, &mut out);
        adsr.tick(&beat, &mut out);
        assert_eq!(adsr.phaser.len(), 2, "two beats should create two voices");
    }

    #[test]
    fn mount_adsr_an_inserts_into_net() {
        let mut net = Net::new(0, 1);
        let generator = Box::new(GeneratorNode::new(DEFAULT_SR));
        let id = mount_adsr_an(&mut net, generator, 2, 20.0, 20_000.0);
        let _ = id;
        // Should not panic.
        net.check();
    }
}
