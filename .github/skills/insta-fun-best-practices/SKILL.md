---
name: insta-fun-best-practices
description: 'Best practices for writing snapshot and raw-data tests with the insta-fun crate for FunDSP AudioUnits. Use when writing tests for audio units, choosing between SVG/WAV snapshot assertions and raw data assertions, configuring SnapshotConfig, setting up InputSource, handling nondeterministic audio, or debugging failing snapshot tests.'
argument-hint: 'Describe your audio unit under test and what you want to assert (snapshot regression, raw data invariants, waveform shape, etc.)'
---

# insta-fun Best Practices

## When to Use

- Writing test coverage for FunDSP `AudioUnit` implementations
- Choosing whether to use `assert_audio_unit_snapshot!` (SVG/WAV regression) or `assert_audio_unit_data!` (raw sample assertions)
- Configuring warmup, processing mode, input source, or output format
- Testing units with nondeterministic output (noise, entropy-driven units, units with random state)
- Debugging snapshot mismatches or abnormal sample panics

---

## Key Decision: Snapshot vs. Raw Data

| Goal | API to use |
|------|-----------|
| Detect rendering regressions visually or audibly | `assert_audio_unit_snapshot!` |
| Assert structural/statistical invariants without brittle pixel-matching | `assert_audio_unit_data!` |
| Nondeterministic unit (noise, randomness, time-varying) | `assert_audio_unit_data!` |
| Deterministic unit — verify exact waveform shape over time | `assert_audio_unit_snapshot!` |
| Extract raw buffers for custom assertions outside insta | `snapshot_audio_unit_data_with_input_and_options` |

---

## Procedure

### 1. Import

Always use the prelude for clean imports:

```rust
use insta_fun::prelude::*;
use fundsp::prelude::*;
```

### 2. Choose an InputSource

| Variant | When to use |
|---------|------------|
| `InputSource::None` | Source unit (oscillators, generators — no input) |
| `InputSource::impulse()` | Impulse response of filters |
| `InputSource::sine(freq, sr)` | Steady-state filter response |
| `InputSource::Generator(Box::new(\|i, ch\| ...))` | Custom per-sample, per-channel data |
| `InputSource::VecByChannel(vec![...])` | Pre-computed exact buffers |
| `InputSource::VecByTick(vec![...])` | Pre-computed tick-oriented data |
| `InputSource::Flat(vec![...])` | Same values repeated every tick |
| `InputSource::AudioUnit(Box::new(unit))` | Drive a unit-under-test with another unit's output |

> `InputSource::AudioUnit(...)` does **not** call `set_sample_rate` or `reset` on the input unit — do that before wrapping.

### 3. Configure SnapshotConfig

```rust
let config = SnapshotConfigBuilder::default()
    .sample_rate(44_100.0)      // default: fundsp::DEFAULT_SR
    .num_samples(1024)           // default: 1024
    .processing_mode(Processing::Tick)   // or Processing::Batch(64)
    .warm_up(WarmUp::None)       // or WarmUp::Samples(n) / WarmUp::Seconds(s)
    .allow_abnormal_samples(false)
    .output_mode(SnapshotOutputMode::SvgChart(SvgChartConfig::default()))
    .build()
    .unwrap();
```

For SVG chart tweaks use `SvgChartConfigBuilder` and inject via `.output_mode(chart_cfg)`. WAV output: `.output_mode(WavOutput::Wav16)`.

### 4. Write the Test

#### Deterministic unit — SVG regression

```rust
#[test]
fn lowpass_impulse_response() {
    assert_audio_unit_snapshot!(
        "lowpass_1k_impulse",
        lowpass_hz(1_000.0, 0.707),
        InputSource::impulse()
    );
}
```

#### Deterministic unit — SVG + custom config

```rust
#[test]
fn sine_chart_config() {
    let chart = SvgChartConfigBuilder::default()
        .chart_title("Sine 440Hz")
        .show_grid(true)
        .svg_width(800)
        .build()
        .unwrap();
    let config = SnapshotConfigBuilder::default()
        .num_samples(2048)
        .output_mode(chart)
        .build()
        .unwrap();
    assert_audio_unit_snapshot!("sine_440", sine_hz::<f32>(440.0), InputSource::None, config);
}
```

#### Nondeterministic unit — raw data assertions

Use `assert_audio_unit_data!` to assert invariants without locking in exact sample values.

```rust
#[test]
fn noise_unit_is_bounded() {
    assert_audio_unit_data!(
        white(),
        InputSource::None,
        SnapshotConfigBuilder::default()
            .num_samples(4096)
            .build()
            .unwrap() => |data: &AudioUnitSnapshotData| {
            assert_eq!(data.output_data.len(), 1);
            assert_eq!(data.output_data[0].len(), data.num_samples);
            let max = data.output_data[0].iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let min = data.output_data[0].iter().cloned().fold(f32::INFINITY, f32::min);
            assert!(max <= 1.0, "max sample exceeded 1.0: {max}");
            assert!(min >= -1.0, "min sample below -1.0: {min}");
        }
    );
}
```

#### Nondeterministic unit — macro (input-only form)

```rust
#[test]
fn custom_excitor_output_shape() {
    assert_audio_unit_data!(
        my_excitor_unit,
        InputSource::AudioUnit(Box::new(sine_hz::<f32>(440.0))) => |data: &AudioUnitSnapshotData| {
            assert_eq!(data.output_data.len(), 2);
            assert!(data.abnormalities.iter().all(|ch| ch.is_empty()));
        }
    );
}
```

#### AudioUnit-driven input (filter + generator source)

```rust
#[test]
fn filter_with_unit_input() {
    let mut input = Box::new(sine_hz::<f32>(440.0)) as Box<dyn AudioUnit>;
    input.set_sample_rate(44_100.0);
    input.reset();
    assert_audio_unit_snapshot!(
        "filter_sine_in",
        lowpass_hz(1_000.0, 0.7),
        InputSource::AudioUnit(input)
    );
}
```

#### Warmup before main capture

```rust
let config = SnapshotConfigBuilder::default()
    .warm_up(WarmUp::Samples(20_000))  // or WarmUp::Seconds(0.5)
    .num_samples(2048)
    .build()
    .unwrap();
```

> Use warmup when the unit has state that needs time to stabilize (e.g. reverb tails, filter transients, delay lines).

#### Abnormal sample handling

```rust
let config = SnapshotConfigBuilder::default()
    .allow_abnormal_samples(true)  // NaN/±Inf become 0 and are recorded in data.abnormalities
    .build()
    .unwrap();
```

Access them from raw data:

```rust
assert_audio_unit_data!(my_unit, InputSource::None, config => |data: &AudioUnitSnapshotData| {
    // data.abnormalities: Vec<Vec<(sample_index, SnapshotAbnormalSample)>>
    assert!(data.abnormalities[0].is_empty(), "unexpected NaN/Inf");
});
```

### 5. Run and Review

```bash
cargo test                          # run; any new snapshot must be accepted
cargo insta review                  # interactively accept/reject SVG/WAV snapshots
cargo insta accept                  # accept all pending
```

---

## Common Pitfalls

| Pitfall | Fix |
|---------|-----|
| Snapshot changes every run | Unit is nondeterministic — switch to `assert_audio_unit_data!` |
| `Input vec size mismatch` panic | `InputSource` channel count must match `unit.inputs()` |
| `AudioUnit` input unit produces silence | `InputSource::AudioUnit` requires manual `set_sample_rate`/`reset` upfront |
| Batch processing diverges from Tick | Expected for many DSP units; test both modes if needed |
| `Clone` required on macro unit | `assert_audio_unit_snapshot!` without config clones the unit for dual SVG+WAV — implement `Clone` or use the single-output config form |
| Panic on NaN/Inf | Set `allow_abnormal_samples(true)` and assert on `data.abnormalities` instead |

---

## API Reference Summary

```
snapshot_audio_unit(unit)
snapshot_audio_unit_with_input(unit, input)
snapshot_audio_unit_with_options(unit, config)
snapshot_audio_unit_with_input_and_options(unit, input, config) → Vec<u8>

snapshot_audio_unit_data(unit)
snapshot_audio_unit_data_with_input(unit, input)
snapshot_audio_unit_data_with_options(unit, config)
snapshot_audio_unit_data_with_input_and_options(unit, input, config) → AudioUnitSnapshotData

assert_audio_unit_snapshot!(unit)
assert_audio_unit_snapshot!("name", unit)
assert_audio_unit_snapshot!("name", unit, input)
assert_audio_unit_snapshot!("name", unit, input, config)
assert_audio_unit_snapshot!(unit, config)

assert_audio_unit_data!(unit, closure)
assert_audio_unit_data!(unit, input => closure)
assert_audio_unit_data!(unit, input, config => closure)

assert_dsp_net_snapshot!("name", net)   // requires feature = "dot"
```

`AudioUnitSnapshotData` fields:

```
input_data:      Vec<Vec<f32>>                            // [channel][sample]
output_data:     Vec<Vec<f32>>                            // [channel][sample]
abnormalities:   Vec<Vec<(usize, SnapshotAbnormalSample)>>
sample_rate:     f64
num_samples:     usize
start_sample:    usize   // first sample index after warmup
```