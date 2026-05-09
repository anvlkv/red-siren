---
description: "Use when adding or changing Rust snapshot tests with insta, and when snapshotting FunDSP audio behavior with insta-fun in crates/audio-system."
applyTo: "{crates,src,src-tauri}/**/*.rs"
---

# Advanced Snapshot Testing

When adding or editing Rust snapshot tests in this workspace, keep to one consistent split.

- Use `insta` for structured or textual snapshots outside DSP behavior. Prefer `assert_json_snapshot!` for serde-serializable values and fall back to `assert_debug_snapshot!` only when there is no stable serialized form.
- In `crates/audio-system`, use `insta_fun::assert_audio_unit_snapshot!` for FunDSP `AudioUnit` behavior such as impulse responses, control sweeps, envelopes, and signal-shape regressions. Do not replace those tests with long raw float assertions unless the behavior is truly scalar.
- Keep snapshots deterministic: fixed sample rates, fixed sample counts, explicit generated inputs, stable snapshot names, and shared local helpers such as `snapshot_config()` or `low_sr_config()` when a module reuses chart settings.
- For `insta_fun`, no explicit `SnapshotConfig` emits both an SVG chart and a WAV16 snapshot; an explicit `SnapshotConfig` emits only the format selected by `output_mode`.
- When snapshot content is noisy or unstable, use `insta` settings, redactions, or filters instead of accepting churn.
- Keep snapshots in the standard nearby `snapshots/` folder and use the normal Insta review flow: generate pending snapshots from tests, review with `cargo insta review` when available, and use `INSTA_UPDATE` intentionally instead of hand-editing snapshot payloads.
- Keep input arity aligned end-to-end. The `InputSource` channel count must match the node/unit input count exactly. Use `InputSource::None` for generators (`U0` inputs), and use `InputSource::VecByChannel(vec![...])` with exactly one channel for `U1` inputs.
- When testing wrappers around generic DSP nodes, ensure the dummy test node has the same input arity assumptions as the wrapper. A wrapper declaring `Inputs = U1` should not be tested with a pure generator like `sine_hz(freq)` (`U0` inputs) unless the generator is adapted to `U1`.

Match the established repo patterns before inventing a new style:

```rust
assert_json_snapshot!(
  format!("config_{}x{}_{:?}", layout.space.x, layout.space.y, layout.scale),
  config
);
```

```rust
fn low_sr_config(num_samples: usize) -> SnapshotConfig {
  SnapshotConfigBuilder::default()
    .sample_rate(100.0)
    .num_samples(num_samples)
    .build()
    .unwrap()
}
```

```rust
assert_audio_unit_snapshot!(
  "grid_bpm_change",
  grid,
  InputSource::Generator(Box::new(|i, _| if i < 150 { 60.0 } else { 120.0 })),
  low_sr_config(400)
);
```

```rust
assert_audio_unit_snapshot!(
  "feedback_pass_catch_sine_loop",
  sine_hz(freq) >> pass | catch,
  InputSource::None,
  snapshot_config()
);
```

```rust
// Wrapper expects U1 input, so adapt sine_hz (U0) to U1 by multiplying with pass().
let node = sine_hz(freq) * pass();
assert_audio_unit_snapshot!(
  "u1_wrapper_dummy_sine",
  node,
  InputSource::VecByChannel(vec![vec![1.0; 256]]),
  snapshot_config(256)
);
```

References:
- https://insta.rs/docs/
- https://docs.rs/insta/latest/insta/
- https://docs.rs/insta-fun/latest/insta_fun/