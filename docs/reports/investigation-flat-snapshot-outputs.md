# Investigation: Flat-Line Snapshot Outputs in `audio-system` Node Tests

**Date:** 2025-05  
**Status:** Root cause identified, fix not yet implemented  
**Affected tests:**
- `mount_node_bands_audio_fully_wired_long_snapshot`
- `mount_band_audio_snapshot`
- `create_channel_bands_audio_snapshot`
- `create_controllers_stack_audio_snapshot`

**Working test (baseline):**
- `create_and_push_band_node_audio_snapshot`

---

## 1. Observed Symptoms

All four failing tests produce SVG snapshots where the output channel(s) (blue `#4285F4`, red `#EA4335`) are **perfectly flat at y = midpoint**, corresponding to output signal = exactly 0.0. The y-axis labels show `"-0.0"` to `"0.0"`, confirming the value is not a scale artefact — the signal is genuinely zero throughout the measurement window.

Input traces ARE visible and non-flat in all four cases:
- `#B39DDB` (hit_strength impulse) spikes at period boundaries
- `#FFAB91` (radius ramp) shows the expected 0.25 → 1.0 sawtooth
- `#FFF59D` (ticks_to_next / rhythm grid) shows the expected countdown

All tests pass (no assertion failures) because the flat outputs were previously accepted as baselines via `cargo insta test --accept`. The snapshots are not wrong by the test runner's judgment, but they represent zero audio output from a system that should be producing sound.

---

## 2. Test Architecture Background

The node tests exercise progressively higher levels of the DSP assembly pipeline:

```
create_controllers_stack  →  produces scheduling ratios (start, duration, accent)
         ↓
create_and_push_band_node →  produces audio (EnvelopedNodeGenerator)
         ↓
mount_band                →  wires controller outputs into band inputs; outputs audio
         ↓
create_channel_bands      →  wraps mount_band for a full channel
         ↓
mount_node_bands          →  stereo L/R channel output
```

`create_and_push_band_node` works correctly because its test bypasses the controller stack entirely: it drives the envelope inputs (`start_ratio`, `duration_ratio`, `control`, `accent`) directly with hard-coded values.

---

## 3. How `insta_fun` Warm-Up Works

`SnapshotConfig::warm_up(WarmUp::Seconds(0.25))` runs 11 025 samples of warm-up **with all inputs set to zero** (`none_input`), then runs `num_samples` (2048–8192) of the actual measurement window using the `InputSource::Generator` drive function.

The generator's sample index `i` is **local** (0 … num_samples−1), restarting from zero at the start of the measurement window.

**Consequence:** No hits are delivered during the warm-up phase; all reverb delay lines remain empty at the start of measurement.

---

## 4. Root Cause: `reverb4_stereo` Delay Lines Exceed Measurement Window

### 4.1 Parameter Computation

In `create_controllers_stack` (node.rs line 396–405):

```rust
let room_size_m3      = node_config.room_size_m3();          // ≈ 30.0 m³ for test node
let reverb_time_to_min60db = node_config.hr_bpm() as f64 / 60.0;  // = 120 / 60 = 2.0 s
```

For a test node created by `NodeConfig::new_test_node(220.0)`:

| Parameter | Value | Derivation |
|---|---|---|
| `w_kg` | ≈ 0.0894 kg | √(W_MIN_KG × W_MAX_KG) = √(0.02 × 0.4) |
| `density` (g/cm³) | 40.0 | √(BODY_DENSITY_MIN × BODY_DENSITY_MAX) = √(2 × 800) |
| `v_cm3` | ≈ 2.24 cm³ | (0.0894 × 1000) / 40 |
| `log_v_norm` | ≈ 0.5 | ln(2.24 / 0.025) / ln(200 / 0.025) |
| `hr_bpm()` | 120 bpm | BPM_MAX × (BPM_MIN/BPM_MAX)^0.5 = 240 × 0.5 |
| `reverb_time_to_min60db` | **2.0 s** | 120 / 60 |
| `room_size_m3()` | **30.0 m³** | ROOM_SIZE_MIN × (MAX/MIN)^0.5 = 15 × √4 |

### 4.2 FunDSP `reverb4_stereo` Internal Delay Lengths

`reverb4_stereo(room_size, time)` (fundsp 0.23, `prelude.rs` line 1873) computes each delay as:

```rust
*delay *= max(room_size as f32, 15.0) / 10.0;
// With room_size = 30.0: *delay *= 3.0
```

The shortest delay in the table is `0.031507637` seconds (at 10 m room):

```
shortest_delay = 0.031507637 × 3.0 = 0.094523 s
               = 0.094523 × 44100 ≈ 4169 samples
```

The longest delay is `0.069954 × 3.0 = 0.2099 s ≈ 9255 samples`.

### 4.3 Mismatch with Measurement Windows

| Test | Measurement window | Min reverb delay | Output |
|---|---|---|---|
| `create_controllers_stack` | **2 048 samples** | 4 169 samples | Zero |
| `mount_band` | **4 096 samples** | 4 169 samples | Zero |
| `create_channel_bands` | **4 096 samples** | 4 169 samples | Zero |
| `mount_node_bands` | **8 192 samples** | 4 169 samples | Zero (see §4.4) |

For the first three tests the measurement window is shorter than the minimum reverb delay; the first hit's output simply never arrives. Since warm-up uses zero input, the delay lines are empty at t=0 of measurement; no output is produced within the window.

### 4.4 Why `mount_node_bands` (8 192 samples) Is Also Flat

Even though the 8 192-sample window is longer than the 4 169-sample minimum delay, the output is still zero because the controller stack's output is used as scheduling parameters (`start_ratio`, `duration_ratio`) fed into `EnvelopedNodeGenerator`. The rhythm grid trigger fires every 96 samples (at i = 0, 96, 192, …). The controller outputs are 0.0 for samples 0–4168, so the envelope fires repeatedly with `start_ratio = 0.0` and `duration_ratio = 0.0`. A `duration_ratio` of 0.0 means a zero-length note, which produces no audio output from the oscillator. By the time the reverb produces its first non-zero output at sample ≈ 4169, all triggers in the 8 192-sample window have already fired with duration 0.

---

## 5. Why `create_and_push_band_node` Works

`create_and_push_band_node_audio_snapshot` drives the envelope directly without going through the controller stack. Its `envelope_band_drive()` sets:

```
ch 3  (start_ratio)    = 0.0
ch 4  (duration_ratio) = 0.5   ← non-zero: note plays for half a beat
ch 5  (control)        = 1.0
ch 6  (accent)         = 1.0
```

With `duration_ratio = 0.5` held constant throughout the window, the envelope fires real notes on every trigger, producing the observed audio output.

---

## 6. Secondary Architectural Issue

`reverb4_stereo` is an audio-rate reverb designed for acoustic room simulation. It is being used here to "blur" scheduling control signals (`start_ratio`, `duration_ratio`) in time. This is architecturally questionable:

1. **Wrong semantics**: A room reverb adds acoustic reflections; these values are musical scheduling ratios (0–4), not audio samples.
2. **Wrong time scale**: Reverb delay lines operate at acoustic room scales (≥ 94 ms). Control signals should update on beat or sub-beat time scales (< 10 ms for a 120 BPM system with 96-sample period ≈ 2.2 ms).
3. **Warm-up sensitivity**: The snapshot warm-up provides zero input, so no reverb tail exists at the start of measurement. Real playback would have some history, but tests do not.

A smoothing operator such as `follow(t)` (already used for accentuation in the same chain) or a sample-and-hold with decay would be more appropriate for this role.

---

## 7. Proposed Fix

### Option A — Replace reverb with per-output `follow` smoothers (recommended)

Change the controller stack pipeline from:

```rust
>> (reverb4_stereo(room_size_m3, reverb_time_to_min60db)
    | follow(reverb_time_to_min60db))
```

to:

```rust
>> (follow(reverb_time_to_min60db)
    | follow(reverb_time_to_min60db)
    | follow(reverb_time_to_min60db))
```

`follow(t)` is a one-pole lowpass (lag filter) with time constant `t` seconds. It smooths step impulses without introducing multi-hundred-millisecond delay lines. The `reverb_time_to_min60db = 2.0 s` time constant would make the ratios decay back to zero in approximately 2 seconds, which is reasonable for a scheduling parameter.

**Impact on snapshots:** The first hit at sample ~15 of the measurement window produces a non-zero `follow` output immediately (decaying exponential), which is visible within a 2048-sample window.

### Option B — Fix warm-up to include the drive signal

Switch to `WarmUp::SamplesWithInput` in `snapshot_config()`:

```rust
WarmUp::SamplesWithInput { samples: 11025, input: controller_drive_static_vec }
```

This would fill the reverb delay lines before measurement. However this is harder to implement with `InputSource::Generator` and doesn't address the semantic mismatch.

### Option C — Reduce room size so delays fit within measurement window

`room_size < 15.0` is clamped to 15.0 by `reverb4_stereo`, giving min delay `0.031507637 × 1.5 = 0.04726 s ≈ 2086 samples`. This would only just fit the 4096-sample windows but not the 2048-sample one. Also semantically wrong (passing a volume in m³ as a linear dimension in meters).

**Recommendation:** Implement Option A. The controller stack should use `follow` for all three outputs, matching the semantic intent of smoothly held control values.

---

## 8. Files Involved

| File | Role |
|---|---|
| [`crates/audio-system/src/system/node.rs`](../crates/audio-system/src/system/node.rs) | `create_controllers_stack` — the reverb is here, line 404 |
| [`crates/audio-system/src/system/node/tests.rs`](../crates/audio-system/src/system/node/tests.rs) | All five snapshot tests |
| [`crates/audio-system/src/system/node/controller.rs`](../crates/audio-system/src/system/node/controller.rs) | `NodeController::tick` — correct; outputs non-zero on hits |
| [`crates/common/src/instrument/config/node.rs`](../crates/common/src/instrument/config/node.rs) | `new_test_node`, `hr_bpm()`, `room_size_m3()` |
| [`crates/common/src/instrument/consts.rs`](../crates/common/src/instrument/consts.rs) | Physical constants (BPM_MIN/MAX, ROOM_SIZE_MIN/MAX_M3, etc.) |
| `~/.cargo/registry/…/fundsp-0.23.0/src/prelude.rs` | `reverb4_stereo` — delay lines line 1873, note comment at line 1718: "room_size is in **meters**" (not m³) |

---

## 9. Summary

The flat-line snapshots are caused by a single root cause: **`reverb4_stereo` is used to process scheduling control signals, and its minimum delay (~4 169 samples, ~94 ms) exceeds the snapshot measurement windows (2 048–4 096 samples)**. Because `insta_fun` warm-up supplies zero input, the reverb delay lines are empty when measurement begins. The controller stack therefore outputs 0.0 for all samples within the window, causing downstream components (`mount_band`, `create_channel_bands`, `mount_node_bands`) to schedule zero-length notes and produce no audio.

The fix is to replace `reverb4_stereo` in `create_controllers_stack` with three `follow(t)` smoothers, one per controller output, which have no latency (one-pole IIR) and match the semantic intent of smoothly decaying scheduling parameters.
