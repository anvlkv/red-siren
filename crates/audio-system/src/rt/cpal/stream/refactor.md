# Playback Callback Refactor Specification

Purpose
- Refactor the CPAL playback callback to:
  - Prefer `tick` in steady state for best quality.
  - Use `process` and `process_big` only to catch up or when latency is too high.
  - Derive buffer targets from `PlaybackQualityGate::buffer_size(...)`.
  - Compute latency from buffer depth and production time, using timestamps only as secondary hints.
  - Keep the `quality` indicator exactly equal to the current `PlaybackQualityGate` (I8-compatible).

Principles
- MAYA: keep implementation simple and acceptable; reuse `quality.rs` selection logic.
- DRY: centralize buffer target and quality computation.
- KISS: separate telemetry (measurement), producer (mode selection), and manager (gate changes).
- Minimal overhead: no overkill; the manager must not introduce noticeable extra latency (lightweight periodic checks; no work on the audio thread beyond simple atomics/locks).
- Lovingly: explain rationale in comments next to implementation changes.

Scope
- `crates/audio-system/src/rt/cpal/stream/playback.rs`: callback mode selection and telemetry.
- `crates/audio-system/src/rt/cpal/engine.rs`: periodic quality gate management and stream reconfiguration.
- `crates/audio-system/src/quality.rs`: unchanged; used as source of truth for sample rate, sample type, buffer size, and gate transitions.

Key Constraints and Alignments
- Quality indicator maps 1:1 to `PlaybackQualityGate` values:
  - Ultra = 2, HiFi = 1, Medium = 0, LoFi = -1
- Sample type must change across quality boundaries:
  - HiFi/Ultra → `F64`
  - Medium/LoFi → `F32`
- Buffer size is determined by `PlaybackQualityGate::buffer_size(...)` (frames), derived from constants and device ranges.
- Fundsp production granularity:
  - `tick`: 1 sample
  - `process`: up to 64 samples
  - `process_big`: arbitrary larger sizes

---

## Design

### 1) Telemetry (measurement-only)

Add a compact telemetry struct shared by the callback and engine. Callback writes; engine reads.

Tracks:
- `sample_rate: u32`
- `last_callback_frames: usize`
- `queue_depth_frames: usize`
- `ema_render_ns_per_frame: f64`
- `ema_compute_slack_ns: f64`
- `net_latency_frames: usize`
- `estimated_latency_ms: f64`
- `local_underruns: u32`
- `total_underruns: u64`

Rationale:
- Latency = queue depth + net latency, converted to ms.
- Slack = expected callback period (from frames/sample_rate) minus measured render time; negative slack indicates falling behind.
- Use EMAs to smooth jitter.

Snippet (type and helpers):

```/dev/null/spec/playback_telemetry.rs#L1-80
use std::time::Duration;

#[derive(Debug, Default, Clone)]
pub struct PlaybackTelemetry {
    pub sample_rate: u32,
    pub last_callback_frames: usize,
    pub queue_depth_frames: usize,

    pub ema_render_ns_per_frame: f64,
    pub ema_compute_slack_ns: f64,

    pub net_latency_frames: usize,
    pub estimated_latency_ms: f64,

    pub local_underruns: u32,
    pub total_underruns: u64,
}

impl PlaybackTelemetry {
    pub fn ema(cur: f64, sample: f64, alpha: f64) -> f64 {
        if cur == 0.0 { sample } else { alpha * sample + (1.0 - alpha) * cur }
    }
    pub fn note_render(&mut self, frames: usize, elapsed: Duration) {
        let ns_per_frame = (elapsed.as_nanos() as f64) / frames as f64;
        self.ema_render_ns_per_frame = Self::ema(self.ema_render_ns_per_frame, ns_per_frame, 0.2);
        self.last_callback_frames = frames;
    }
    pub fn recompute_slack(&mut self, expected_period: Duration) {
        let expected_ns = expected_period.as_nanos() as f64;
        let render_ns = self.ema_render_ns_per_frame * self.last_callback_frames as f64;
        let slack = expected_ns - render_ns;
        self.ema_compute_slack_ns = Self::ema(self.ema_compute_slack_ns, slack, 0.3);
    }
    pub fn recompute_latency(&mut self) {
        let q_ms = (self.queue_depth_frames as f64 / self.sample_rate as f64) * 1000.0;
        let net_ms = (self.net_latency_frames as f64 / self.sample_rate as f64) * 1000.0;
        self.estimated_latency_ms = q_ms + net_ms;
    }
}
```

Callback integration points:
- Measure start/end of production per callback to call `note_render(...)`.
- Update `queue_depth_frames` with `output_buffer.len()`.
- Update `net_latency_frames` from `BigBlockAdapter::latency().ceil()`.
- After each callback, compute `estimated_latency_ms` and `ema_compute_slack_ns`.

### 2) Producer (mode selection)

Goals:
- Prefer `tick` to fill toward a steady buffer depth.
- Use `process/process_big` only to catch up or when latency is too high.

Definitions:
- `buffer_target_frames = PlaybackQualityGate::buffer_size(cfg_range)` → frames.
- `steady_depth = round(buffer_target_frames * 0.8)` → 80% occupancy.
- `buffer_duration_ms = buffer_target_frames / sample_rate * 1000`.
- `estimated_latency_ms = (queue_depth_frames + net_latency_frames) / sample_rate * 1000`.

Enter catch-up when any of:
- `local_underruns > 0`
- `ema_compute_slack_ns < 0` (consider consecutive callbacks for stability)
- `estimated_latency_ms > buffer_duration_ms * 0.9`

Fill size:
- Normal: `toward_steady = max(steady_depth - current_depth, 0)`
- Catch-up:
  - `deficit = steady_depth - current_depth`
  - `extra = frames_from_ns((-ema_compute_slack_ns).max(0))`
  - `fill_size = max(num_frames, deficit + extra)`
- Clamp by remaining capacity.

Mode selection:
- `fill_size <= 4` → `tick`
- `5..=64` → `process`
- `>64` → `process_big`

Snippet (decision logic):

```/dev/null/spec/playback_mode_decision.rs#L1-100
fn frames_from_ns(ns: f64, sample_rate: u32) -> usize {
    ((ns / 1_000_000_000.0) * sample_rate as f64).round() as usize
}

struct Decision {
    fill_size: usize,
    mode: Mode,
    need_catch_up: bool,
}

enum Mode { Tick, Process, ProcessBig }

fn decide(
    num_frames: usize,
    sample_rate: u32,
    buffer_target_frames: usize,
    current_depth: usize,
    net_latency_frames: usize,
    ema_compute_slack_ns: f64,
    local_underruns: u32,
) -> Decision {
    let steady_depth = ((buffer_target_frames as f64) * 0.8).round() as usize;
    let buffer_duration_ms = (buffer_target_frames as f64 / sample_rate as f64) * 1000.0;
    let estimated_latency_ms = ((current_depth + net_latency_frames) as f64 / sample_rate as f64) * 1000.0;

    let need_catch_up = local_underruns > 0
        || ema_compute_slack_ns < 0.0
        || estimated_latency_ms > buffer_duration_ms * 0.9;

    let fill_size = if need_catch_up {
        let deficit = steady_depth.saturating_sub(current_depth);
        let extra = frames_from_ns((-ema_compute_slack_ns).max(0.0), sample_rate);
        (deficit + extra).max(num_frames)
    } else {
        steady_depth.saturating_sub(current_depth).max(num_frames)
    };

    let mode = if fill_size <= 4 {
        Mode::Tick
    } else if fill_size <= 64 {
        Mode::Process
    } else {
        Mode::ProcessBig
    };

    Decision { fill_size, mode, need_catch_up }
}
```

Production:
- In `Mode::Tick`: loop `fill_size` times calling `tick`.
- In `Mode::Process`: allocate scratch `fill_size` (≤64), call `process`.
- In `Mode::ProcessBig`: resize scratch as needed, call `process_big`.

### 3) Quality indicator equals gate

- The per-callback `quality` atomic should store the current `PlaybackQualityGate` as `i8`:
  - `quality.store(gate as i8, Ordering::Relaxed)`
- The callback does not permanently change the gate; it only reflects current gate value.
- The engine owns raising/lowering the gate.

### 4) Gate management in engine

- Gate switching is allowed only when the “automatic quality” flag is enabled. If auto is disabled, the user-selected gate remains active.
- The engine still computes and stores a “recommended gate” based on telemetry, even when auto is off, so the UI can suggest upgrades/downgrades without forcing them.

Policy (checked periodically, e.g., every 200–500 ms, off the audio thread):
- Degrade quickly if:
  - `ema_compute_slack_ns < -3 ms` for > 1 s, or
  - repeated underruns over last N callbacks.
- Upgrade slowly if:
  - `ema_compute_slack_ns ≥ +2 ms` and zero underruns for ≥ 20 s.
- Apply cooldown (e.g., 5 s) between changes to avoid flapping.
- When auto is ON and a change is warranted:
  - Recreate streams using `PlaybackQualityGate::select_output_config(...)`.
  - Use `PlaybackQualityGate::buffer_size(...)` for target frames.
  - Switch `SampleType` per `PlaybackQualityGate::sample_type()` (F32/F64).
  - Rebuild fundsp net if sample type boundary crossed (Medium/LoFi↔HiFi/Ultra).
- When auto is OFF:
  - Update the “recommended gate” only; do not reconfigure streams.

Minimal overhead:
- Evaluation runs in the controller thread; avoid heavy computations.
- No extra work on the audio thread besides reading/writing lightweight telemetry and atomics.

Snippet (engine-side evaluation):

```/dev/null/spec/gate_manager.rs#L1-140
use std::time::{Duration, Instant};
use crate::quality::PlaybackQualityGate;

pub struct GateManager {
    last_change: Instant,
    cooldown: Duration,
    stable_since: Option<Instant>,
    pub recommended_gate: PlaybackQualityGate,
}

impl GateManager {
    pub fn new(initial_gate: PlaybackQualityGate) -> Self {
        Self {
            last_change: Instant::now(),
            cooldown: Duration::from_secs(5),
            stable_since: None,
            recommended_gate: initial_gate,
        }
    }

    pub fn evaluate_recommendation(
        &mut self,
        current_gate: PlaybackQualityGate,
        ema_compute_slack_ns: f64,
        recent_underruns: bool,
        now: Instant,
    ) -> Option<PlaybackQualityGate> {
        let slack_ms = ema_compute_slack_ns / 1_000_000.0;
        let can_change = now.duration_since(self.last_change) >= self.cooldown;

        // Degrade fast on trouble
        if can_change && (slack_ms < -3.0 || recent_underruns) {
            let new_gate = current_gate.lower();
            if new_gate != current_gate {
                self.last_change = now;
                self.stable_since = None;
                self.recommended_gate = new_gate;
                return Some(new_gate);
            }
        }

        // Upgrade slowly on stability
        let stable = slack_ms >= 2.0 && !recent_underruns;
        if stable {
            self.stable_since.get_or_insert(now);
        } else {
            self.stable_since = None;
        }

        if can_change {
            if let Some(since) = self.stable_since {
                if now.duration_since(since) >= Duration::from_secs(20) {
                    let new_gate = current_gate.higher();
                    if new_gate != current_gate {
                        self.last_change = now;
                        self.stable_since = None;
                        self.recommended_gate = new_gate;
                        return Some(new_gate);
                    }
                }
            }
        }

        // No change; keep previous recommendation
        None
    }
}
```

### 5) Timestamp reliability guard

- Use `OutputStreamTimestamp.callback` for expected pacing; compute expected period as `frame_to_duration(num_frames, sample_rate)`.
- Treat `playback` as a hint; if it drifts beyond buffer duration consistently, mark unreliable and stick to computed period.
- Cross-check device pacing with buffer depth changes; if mismatched, rely on measured render time and steady-depth target rather than playback timestamp.

---

## Callback Flow Overview

1) Read `num_frames` for this callback.
2) Write out frames from `output_buffer`, counting `local_underruns`.
3) Update silence accumulator; reset net if exceeding threshold (existing behavior).
4) Update telemetry:
   - `queue_depth_frames = output_buffer.len()`
   - `net_latency_frames = adapter.latency().ceil() as usize` if available
   - Measure production elapsed and call `note_render(...)`
   - Compute slack via expected callback period
   - Recompute estimated latency
5) Decide fill and mode using the decision logic:
   - Prefer `tick` normally towards `steady_depth = 80% * buffer_target_frames`
   - Use `process/process_big` only if catching up
6) Produce new frames using chosen mode and push to `output_buffer`.
7) Set quality indicator from gate:
   - `quality.store(current_gate as i8, Relaxed)`

Short illustrative snippet (inside callback, pseudocode-format):

```/dev/null/spec/callback_flow.rs#L1-120
let num_frames = l_frames[0].len();
telemetry.queue_depth_frames = output_buffer.len();
telemetry.net_latency_frames = adapter.latency().map(|d| d.ceil() as usize).unwrap_or(0);

let expected_period = frame_to_duration(num_frames, sample_rate);

// measure production elapsed around fill (start->end)
let start = Instant::now();

let Decision { fill_size, mode, need_catch_up } = decide(
    num_frames,
    sample_rate,
    buffer_target_frames,            // from PlaybackQualityGate::buffer_size(...)
    output_buffer.len(),
    telemetry.net_latency_frames,
    telemetry.ema_compute_slack_ns,
    local_underruns,
);

match mode {
    Mode::Tick => {
        for _ in 0..fill_size {
            let input = input_buffer.as_ref().and_then(|ib| ib.pop()).unwrap_or_default();
            adapter.tick(&[input], &mut lr_frame_scratch);
            output_buffer.push_back((lr_frame_scratch[0], lr_frame_scratch[1]));
        }
    }
    Mode::Process => {
        ensure_scratch(fill_size);
        fill_input_batch(input_buffer, &mut i_batch_scratch[..fill_size]);
        adapter.process(fill_size, &[&i_batch_scratch[..fill_size]], &mut [ &mut l_batch_scratch[..fill_size], &mut r_batch_scratch[..fill_size] ]);
        extend_output(&mut output_buffer, &l_batch_scratch[..fill_size], &r_batch_scratch[..fill_size]);
    }
    Mode::ProcessBig => {
        ensure_scratch(fill_size);
        fill_input_batch(input_buffer, &mut i_batch_scratch[..fill_size]);
        adapter.process_big(fill_size, &[&i_batch_scratch[..fill_size]], &mut [ &mut l_batch_scratch[..fill_size], &mut r_batch_scratch[..fill_size] ]);
        extend_output(&mut output_buffer, &l_batch_scratch[..fill_size], &r_batch_scratch[..fill_size]);
    }
}

let elapsed = start.elapsed();
telemetry.note_render(num_frames, elapsed);
telemetry.recompute_slack(expected_period);
telemetry.recompute_latency();

// indicator equals gate
quality.store(current_gate as i8, std::sync::atomic::Ordering::Relaxed);
```

---

## Engine Integration

- Maintain `Arc<RwLock<PlaybackQualityGate>>` for the current gate.
- Periodically read telemetry and evaluate gate changes via `GateManager`.
- On change:
  - Shutdown and recreate streams using selected config (device, sample rate, buffer size).
  - Rebuild the fundsp net with appropriate sample type per gate.

---

## Tuning Parameters

- Steady occupancy: 80% of buffer frames (adjustable: 75–90% based on device jitter tolerance).
- Catch-up entering threshold:
  - Any underruns in callback
  - Negative slack EMA
  - Estimated latency exceeding 90% of buffer duration
- Gate change thresholds (engine):
  - Degrade: slack < -3 ms for > 1 s or repeated underruns
  - Upgrade: slack ≥ +2 ms, zero underruns, sustained for ≥ 20 s
  - Cooldown: 5 s after any gate change

---

## Notes

- The callback does not transition the gate persistently; it only reports the current gate as the indicator. Configuration changes are centralized in the engine, which owns gate transitions and writes the indicator atomic on changes.
- Automatic gate switching occurs only when the auto flag is ON. When auto is OFF, the engine still tracks and stores a recommended gate for UI/UX, but does not apply it.
- Keep overhead minimal: the audio thread performs only simple atomic writes and brief lock reads/writes; evaluation and reconfiguration run off the audio thread.
- `OutputStreamTimestamp.playback` remains a secondary signal; computed expected period and measured render time are primary inputs.
- Prefer `tick` for best temporal fidelity and quality; batch modes are reserved for recovery.

## Implementation Status (in-progress)

Phase 1 — Telemetry
- Added `rt/cpal/stream/telemetry.rs` with:
  - `PlaybackTelemetry` fields: `sample_rate`, `last_callback_frames`, `queue_depth_frames`, `ema_render_ns_per_frame`, `ema_compute_slack_ns`, `net_latency_frames`, `estimated_latency_ms`, `local_underruns`, `total_underruns`.
  - Helpers: `ema`, `note_render`, `recompute_slack`, `recompute_latency`, `frames_from_ns`, `note_underruns`.
  - Tuning consts centralized: steady occupancy (80%), catch-up ratio (90%), gate thresholds and cooldown.
- Callback integration:
  - Updates telemetry each callback: queue depth, sample rate, net latency frames.
  - Measures production elapsed precisely around mode-selected production; calls `note_render`, `recompute_slack`, and `recompute_latency`.
- Engine wiring:
  - Creates shared `Arc<Mutex<PlaybackTelemetry>>` and passes it to the callback.
  - Stores telemetry handle in the controller for engine-thread access.

Phase 2 — Producer (mode selection)
- Buffer target is gate-derived:
  - `buffer_target_frames = PlaybackQualityGate::buffer_size(...)` (currently called with `None`; future: pass `SupportedStreamConfigRange` if available).
  - Steady-depth uses 80% of target.
- Decision logic implemented (`decide(...)`):
  - Catch-up triggers on any underruns, negative slack EMA, or estimated latency > 90% of buffer duration.
  - Mode selection prefers `tick` for ≤4 frames; batch uses `process_big` for all batch sizes to match adapter signature.
- Production paths:
  - Tick loops per-frame.
  - Batch uses `process_big` with resized scratch buffers.
- Playback cleanup:
  - Removed legacy constants/helpers (`BASE_OPTIMAL_BUFFER_MILLIS`, `duration_to_frames`).
  - Removed redundant second production pass; telemetry updates occur once per callback.
  - Callback reads `quality` atomic but does not write it.

Phase 3 — Quality Indicator Equals Gate
- Engine owns writing `quality_indicator` on gate changes; callback reflects it read-only.

Phase 4 — Gate Management (engine)
- `GateManager` implemented with thresholds, stability windows, and cooldowns.
- Engine-thread periodic evaluation added:
  - A loop in `AudioRuntime::start` calls `evaluate_gate_and_maybe_restart()` every ~300 ms.
  - On recommendation (and `auto_quality` ON), controller:
    - Fades out and shuts down streams.
    - Updates `quality_gate` and `quality_indicator`.
    - Restarts via existing `start(...)` flow using last stored layout/config/source/tuner_config.
    - Fades in and resets GateManager timers.
- Removed owner-side `ReconfigureGate` control path; reconfiguration uses shutdown-and-restart.

Phase 5 — Timestamp Reliability Guard (implemented)
- The callback measures render time per invocation and computes slack against the computed expected period (from frame count and sample rate).
- A lightweight guard marks device timestamps unreliable when repeated underruns or buffer-duration overshoot occurs; when unreliable, computed period is preferred for slack/latency decisions.

Known technical deltas and TODOs
- Small-batch `process` path is implemented using Fundsp `BufferRef/BufferMut` for ≤64 samples; larger batches continue to use `process_big`. This restores the intended production granularity without custom scratch slices.
- Minor clippy/warnings to tidy:
  - Remove any remaining unused variables and duplicate computations.
  - Optionally fold callback parameters into a small config struct if “too many arguments” appears.
- Gate indicator invariant:
  - Ensure engine writes `quality.store(current_gate as i8, Relaxed)` only on gate changes; callback remains read-only.

---