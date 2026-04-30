# Feedback Resonance + Summary Blend Implementations

## Problem framing

You asked for practical implementations of two TODOs in the new Excitor path:
- `excitement_from_feedback`
- `summary_excitement`

Chosen constraints:
- sympathetic resonance focus
- weighted-sum fusion for spectrum + feedback
- output clamped to `[0, 1]`
- imaginary channel also clamped to `[0, 1]`

Interpretation used below:
- `re`: resonance strength
- `im`: spatial/position-like descriptor

## Codebase inspiration

Key facts from current code:
- `SensorData` has `min_frequency`, `max_frequency`, `min_magnitude`, `max_magnitude`, and probe helpers.
- `NodeConfig` exposes physical parameters: `frequency`, `l_mm`, `w_kg`, `v_cm3`, and derived methods.
- `excitement_from_spectrum` already emits `Complex<S>` and keeps values normalized around `[0, 1]` semantics.
- `tick` currently computes both analysis and feedback paths, then merges with `summary_excitement`.

Relevant files:
- `crates/audio-system/src/system/excitor.rs`
- `crates/common/src/tuner/sensor.rs`
- `crates/common/src/instrument/config/node.rs`

## Web inspiration

Practical DSP heuristics used:
- sympathetic resonance is strongest near shared/close frequencies and harmonics.
- damped-oscillator amplitude shape is well approximated by a detuning falloff term.
- simple real-time form: amplitude proportional to input energy and inversely proportional to normalized detune.

References:
- https://en.wikipedia.org/wiki/Sympathetic_resonance
- https://en.wikipedia.org/wiki/Harmonic_oscillator
- https://en.wikipedia.org/wiki/Damping_ratio

## Idea options (3+)

### Option 1: Band-overlap + normalized gain (lowest risk)

Concept:
- Map each sensor to a resonance gate based on overlap of node base frequency with sensor frequency band.
- Real channel = normalized node sample scaled by overlap and by node physical factor.
- Imag channel = position-like measure from how close node frequency is to the sensor-band center.

Fit:
- Uses only fields already present in `SensorData` + `NodeConfig`.
- Keeps all values in `[0, 1]` with explicit clamp.

Feasibility:
- High.

Risks:
- Coarse frequency relation (no harmonic series).

Effort:
- Low.

Sketch:

```rust
fn excitement_from_feedback(
    feedback_data: &[f32],
    tuner_config: &TunerConfig,
    instrument_config: &InstrumentConfig,
) -> Vec<Complex<S>> {
    tuner_config
        .sensor_data
        .iter()
        .map(|sensor_cfg| {
            let node_cfg = instrument_config
                .get_node(&sensor_cfg.key)
                .expect("instrument and tuner data configuration mismatch");
            let index = instrument_config
                .get_node_index(&sensor_cfg.key)
                .expect("instrument and tuner data configuration mismatch");
            let sample = feedback_data
                .get(index)
                .expect("feedback data length must match the number of nodes in the instrument configuration");

            let min_f = sensor_cfg.min_frequency.max(1.0);
            let max_f = sensor_cfg.max_frequency.max(min_f + f32::EPSILON);
            let center_f = 0.5 * (min_f + max_f);
            let half_bw = 0.5 * (max_f - min_f).max(f32::EPSILON);

            let node_f = node_cfg.frequency as f32;
            let detune = ((node_f - center_f).abs() / half_bw).clamp(0.0, 1.0);
            let overlap = if (min_f..=max_f).contains(&node_f) {
                1.0
            } else {
                (1.0 - detune).max(0.0)
            };

            let physical = ((node_cfg.v_cm3 as f32).ln_1p() / 8.0).clamp(0.0, 1.0);
            let strength = (sample.abs() * overlap * (0.5 + 0.5 * physical)).clamp(0.0, 1.0);
            let position_like = detune.clamp(0.0, 1.0);

            Complex::new(S::from_f32(strength), S::from_f32(position_like))
        })
        .collect()
}

fn summary_excitement(spectrum: Vec<Complex<S>>, feedback: Vec<Complex<S>>) -> Vec<Complex<S>> {
    let w_s = S::from_f32(0.75);
    let w_f = S::from_f32(0.25);
    let one = S::from_f32(1.0);
    let zero = S::from_f32(0.0);

    spectrum
        .into_iter()
        .zip(feedback)
        .map(|(s, f)| {
            let re = (s.re * w_s + f.re * w_f).min(one).max(zero);
            let im = (s.im * w_s + f.im * w_f).min(one).max(zero);
            Complex::new(re, im)
        })
        .collect()
}
```

### Option 2: Harmonic-likeness resonance (best musical behavior)

Concept:
- Evaluate several harmonics of each node (`h = 1..=H`) against sensor band center.
- Convert detune to coupling via Lorentzian-like falloff: `1 / (1 + (detune / gamma)^2)`.
- Real channel = sample energy times best harmonic coupling.
- Imag channel = distance-from-center proxy for impact position semantics.

Fit:
- Directly reflects sympathetic resonance behavior and your selected goals.

Feasibility:
- High in current tick path (small constant loop per sensor).

Risks:
- Requires tuning `H` and `gamma`.

Effort:
- Medium-low.

Sketch:

```rust
fn excitement_from_feedback(
    feedback_data: &[f32],
    tuner_config: &TunerConfig,
    instrument_config: &InstrumentConfig,
) -> Vec<Complex<S>> {
    const HARMONICS: usize = 6;
    const GAMMA: f32 = 0.18;

    tuner_config
        .sensor_data
        .iter()
        .map(|sensor_cfg| {
            let node_cfg = instrument_config
                .get_node(&sensor_cfg.key)
                .expect("instrument and tuner data configuration mismatch");
            let index = instrument_config
                .get_node_index(&sensor_cfg.key)
                .expect("instrument and tuner data configuration mismatch");
            let sample = feedback_data
                .get(index)
                .expect("feedback data length must match the number of nodes in the instrument configuration");

            let min_f = sensor_cfg.min_frequency.max(1.0);
            let max_f = sensor_cfg.max_frequency.max(min_f + f32::EPSILON);
            let center_f = 0.5 * (min_f + max_f);
            let half_bw = 0.5 * (max_f - min_f).max(f32::EPSILON);
            let base_f = node_cfg.frequency as f32;

            let best_coupling = (1..=HARMONICS)
                .map(|h| {
                    let hf = base_f * h as f32;
                    let detune_norm = ((hf - center_f).abs() / center_f.max(1.0)).clamp(0.0, 4.0);
                    1.0 / (1.0 + (detune_norm / GAMMA).powi(2))
                })
                .fold(0.0_f32, f32::max)
                .clamp(0.0, 1.0);

            let physical_q = ((node_cfg.w_kg as f32).recip() * 10.0).clamp(0.2, 1.0);
            let strength = (sample.abs() * best_coupling * physical_q).clamp(0.0, 1.0);

            let nearest_detune = ((base_f - center_f).abs() / half_bw).clamp(0.0, 1.0);
            let position_like = nearest_detune;

            Complex::new(S::from_f32(strength), S::from_f32(position_like))
        })
        .collect()
}

fn summary_excitement(spectrum: Vec<Complex<S>>, feedback: Vec<Complex<S>>) -> Vec<Complex<S>> {
    let w_s = S::from_f32(0.7);
    let w_f = S::from_f32(0.3);
    let one = S::from_f32(1.0);
    let zero = S::from_f32(0.0);

    spectrum
        .into_iter()
        .zip(feedback)
        .map(|(s, f)| {
            let re = (s.re * w_s + f.re * w_f).min(one).max(zero);
            let im = (s.im * w_s + f.im * w_f).min(one).max(zero);
            Complex::new(re, im)
        })
        .collect()
}
```

### Option 3: Resonance + temporal smoothing in summary stage (most stable)

Concept:
- Keep feedback extraction simple.
- Put perceptual stability in `summary_excitement`: weighted sum + nonlinear soft-knee map.
- Optional one-pole smoothing can be inserted later in `tick` state if needed.

Fit:
- Minimal branchy logic in `excitement_from_feedback`.
- Better against jitter from FFT/feedback mismatch.

Feasibility:
- High.

Risks:
- Less physically explicit in feedback stage.

Effort:
- Low.

Sketch:

```rust
fn excitement_from_feedback(
    feedback_data: &[f32],
    tuner_config: &TunerConfig,
    instrument_config: &InstrumentConfig,
) -> Vec<Complex<S>> {
    tuner_config
        .sensor_data
        .iter()
        .map(|sensor_cfg| {
            let node_cfg = instrument_config
                .get_node(&sensor_cfg.key)
                .expect("instrument and tuner data configuration mismatch");
            let index = instrument_config
                .get_node_index(&sensor_cfg.key)
                .expect("instrument and tuner data configuration mismatch");
            let sample = feedback_data
                .get(index)
                .expect("feedback data length must match the number of nodes in the instrument configuration");

            let min_f = sensor_cfg.min_frequency.max(1.0);
            let max_f = sensor_cfg.max_frequency.max(min_f + f32::EPSILON);
            let center_f = 0.5 * (min_f + max_f);
            let half_bw = 0.5 * (max_f - min_f).max(f32::EPSILON);

            let detune = ((node_cfg.frequency as f32 - center_f).abs() / half_bw).clamp(0.0, 1.0);
            let coupling = (1.0 - detune * detune).max(0.0);

            let re = (sample.abs() * coupling).clamp(0.0, 1.0);
            let im = detune;
            Complex::new(S::from_f32(re), S::from_f32(im))
        })
        .collect()
}

fn summary_excitement(spectrum: Vec<Complex<S>>, feedback: Vec<Complex<S>>) -> Vec<Complex<S>> {
    let w_s = S::from_f32(0.8);
    let w_f = S::from_f32(0.2);
    let one = S::from_f32(1.0);
    let zero = S::from_f32(0.0);

    spectrum
        .into_iter()
        .zip(feedback)
        .map(|(s, f)| {
            let lin_re = (s.re * w_s + f.re * w_f).min(one).max(zero);
            let lin_im = (s.im * w_s + f.im * w_f).min(one).max(zero);

            // Soft-knee companding for smoother control near top-end.
            let re = lin_re / (lin_re + S::from_f32(0.25));
            let im = lin_im / (lin_im + S::from_f32(0.25));
            Complex::new(re.min(one).max(zero), im.min(one).max(zero))
        })
        .collect()
}
```

## Idea comparison

| Option | Sonic realism | Stability | CPU cost | Integration risk |
|---|---:|---:|---:|---:|
| 1: Band-overlap | Medium | High | Low | Low |
| 2: Harmonic-likeness | High | Medium-High | Medium | Medium-Low |
| 3: Summary soft-knee | Medium | Highest | Low | Low |

## Recommendation

Best fit for your selected goals: Option 2.

Why:
- Most faithful to sympathetic resonance behavior.
- Still real-time friendly with a tiny fixed harmonic loop.
- Keeps your chosen weighted-sum summary and clamp invariants.

Suggested initial constants:
- `HARMONICS = 6`
- `GAMMA = 0.18`
- summary weights: `spectrum = 0.7`, `feedback = 0.3`

## Quick validation plan

1. Unit test: exact frequency-aligned sensor should produce higher `re` than detuned sensor.
2. Unit test: outputs always satisfy `0 <= re <= 1` and `0 <= im <= 1`.
3. Snapshot test in `audio-system`: compare no-feedback, mid-feedback, high-feedback impulse scenarios.
4. Sweep `GAMMA` (`0.12, 0.18, 0.28`) and pick the most controllable response.

## References

- https://en.wikipedia.org/wiki/Sympathetic_resonance
- https://en.wikipedia.org/wiki/Harmonic_oscillator
- https://en.wikipedia.org/wiki/Damping_ratio
