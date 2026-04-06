# audio-system

Low-latency audio engine for Red Siren. Built on [FunDSP] for signal-graph
processing and [CPAL] for device I/O.

---

## Crate layout

```
src/
  lib.rs                  – public re-exports
  quality.rs              – PlaybackQualityGate, SampleType
  system/                 – DSP node factories (instrument + tuner)
  rt/
    mod.rs                – AudioRuntime trait, ExcitementSource, NullController
    rt_subsystem.rs       – RuntimeSubsystem (owns the live FunDSP graph)
    telemetry.rs          – per-callback metrics + PlaybackTelemetry decision engine
    gate_manager.rs       – QualityGateManager (background Tokio task)
    cpal/
      engine.rs           – CpalController (native backend)
      stream/
        playback.rs       – output callback + decide() mode selector
        input.rs          – input capture thread
```

---

## Runtime backends

The `AudioRuntime` trait is the single interface all higher layers use. A
concrete backend is selected at compile time via Cargo features:

| Feature       | Backend            |
|---------------|--------------------|
| `rt_cpal`     | `CpalController`   |
| `rt_web`      | `WebController`    |
| _(none)_      | `NullController`   |

`make_stream_controller(telemetry, preset, output_device, input_device)` returns
a `Box<dyn AudioRuntime + Send + Sync>` for whichever backend is compiled in.

---

## DSP graph (`RuntimeSubsystem`)

The live signal graph is split into two independent sub-networks stitched
together inside a root `Net`:

```
                  ┌─────────────────────────────────────┐
                  │           Main Net (0-in, N-out)     │
                  │                                      │
  mic / entropy ──► Tuner Net (1-in, 1-out)  ──tap──►   │
                  │         │ excitement                  │
                  │         ▼                             │
                  │ Instrument Net (0-in, N-out) ────────►│──► speakers
                  └─────────────────────────────────────┘
```

- **Instrument net** — N-channel output; one siren node per key, each with band
  and key `Shared` controls, excitement snoops, and output snoops.
- **Tuner net** — mono in/out; runs the Nyquist preamp, FFT analyser, and
  excitement routing. Controlled entirely via live `Shared` params (no rebuild
  needed for tuner-config updates).
- **Tap** — `tuner_tap_gain_param` routes the tuner's mono output into the
  output mix when non-zero (`start_tap_tuner_audio` / `stop_tap_tuner_audio`).

### Live-update vs. rebuild

Most runtime changes are applied without rebuilding the graph:

| Change | Mechanism | Cost |
|--------|-----------|------|
| Band / key control value | `Shared::set_value()` | free |
| Nyquist threshold / wet-ratio | `Shared::set_value()` | free |
| Frequency range & sensor bounds | `Shared::set_value()` | free |
| Tuner tap on/off | `Shared::set_value()` | free |
| Buffer-size (quality) adaptation | `Arc<RwLock<PlaybackQualityGate>>` read each callback | free |
| Layout / config change | `replace_network()` on instrument subnet | fade + commit |
| Excitement source switch | `replace_tuner_for_source()` + CPAL-only restart | fade + commit, no DSP rebuild |
| Sample-type change (F32 ↔ F64) | full DSP rebuild + CPAL restart | unavoidable |

`replace_network(unit, node_id)` is the shared primitive for in-place subnet
swaps: it fades out the gain, calls `Net::replace()` + `commit()` to push the
new graph to the running backend, then fades back in. When no backend is
attached (stream stopped) it updates the net silently so the next `backend()`
call reflects the change.

---

## Excitement sources

```rust
pub enum ExcitementSource {
    Entropy,  // internal pseudo-random noise — no mic required
    Mic,      // live microphone input via the CPAL input stream
}
```

Switching sources calls `replace_tuner_for_source(new_source)` which rebuilds
only the tuner sub-network (entropy path uses a `RandomExcitor` node; mic path
wires a `Snoop` backend for the input snoop API). The CPAL output stream is then
restarted via `restart_output_stream_keep_runtime` to capture the updated input
buffer — the instrument DSP graph is untouched.

---

## Quality levels

```
PlaybackQualityGate  SampleType  Sample rate  Target buffer
────────────────────────────────────────────────────────────
LoFi                 F32         32 kHz        256 ms
Medium  (default)    F32         44.1 kHz      128 ms
HiFi                 F64         48 kHz         64 ms
Ultra                F64         96 kHz         32 ms
```

`PlaybackQuality` (exposed to the UI via `common`) wraps the gate or carries
`Auto(i8)` when automatic mode is enabled. The `i8` maps directly to the gate
ladder (−1 = LoFi, 0 = Medium, 1 = HiFi, 2 = Ultra).

---

## Playback callback

The hot path lives in `rt/cpal/stream/playback.rs`. Each CPAL callback:

1. **Drains** pre-computed frames from an output `VecDeque` into the device
   buffer.
2. **Decides** how many frames to compute and in which mode:

   ```
   decide(cb_buffer_size, optimal_buffer_size, remaining_filled, remaining_cap)
          → Decision { fill_size, mode }
   ```

   | Mode         | Condition                                         | DSP call              |
   |--------------|---------------------------------------------------|-----------------------|
   | `None`       | output buffer already full                        | —                     |
   | `Tick`       | small deficit or tiny cap remaining               | `AudioUnit::tick()`   |
   | `Process`    | medium deficit, fits in `MAX_BUFFER_SIZE`         | `BigBlockAdapter::process()` |
   | `ProcessBig` | large deficit or both delta and deficit are large | `BigBlockAdapter::process_big()` |

3. **Fills** `fill_size` frames via the chosen mode.
4. **Reports** a `telemetry::Message` (mode, filled/buffer sizes, processing
   time, estimated latency, current gate and sample type) via a lock-free
   `thingbuf` channel.

`buffer_target_frames` is re-read from the live `Arc<RwLock<PlaybackQualityGate>>`
at the top of every invocation so buffer-size adaptations take effect immediately
with no stream restart.

---

## Telemetry and quality gating

```
playback_callback ──thingbuf──► GateWorker (async) ──► PlaybackTelemetry ──► managed_change
```

### `PlaybackTelemetry`

Maintains a rolling **3-second** history of `telemetry::Message` values.
Once the history is full it evaluates three windows on every new message:

| Window   | Duration | Purpose                                          |
|----------|----------|--------------------------------------------------|
| Sustain  | 0.75 s   | Recent mixed quality → veto any change           |
| Degrade  | 0.30 s   | High load recently → emit `quality.lower()`      |
| Upgrade  | 2.25 s   | Sustained low load → emit `quality.higher()`     |

Degrade fires when the processing-to-buffer ratio exceeds **85 %** over the
degrade window. Upgrade fires when it stays below **60 %** over the upgrade
window. The asymmetry (short degrade, long upgrade) means the engine reacts
quickly to distress but waits for a sustained calm period before improving.

### `QualityGateManager`

Owns a dedicated single-thread Tokio runtime so the gate loop never depends on
the Tauri async executor. The `GateWorker` task:

1. Receives messages from `TelemetryReceiver`.
2. Feeds them to `PlaybackTelemetry::accept_message()`.
3. If a gate change is recommended **and** `PlaybackQuality` is `Auto`, writes
   the new gate to `proposed_quality_gate` and calls `managed_change(new_gate)`.

### `managed_change` in `InstrumentState`

```
managed_change(new_gate)
  ├─ always:  stream_controller.update_quality_setting(new_gate)
  │              └─ writes Arc<RwLock<PlaybackQualityGate>>
  │                   └─ playback callback reads it next tick  (zero restart)
  │
  └─ if sample_type changed && stream is running:
       std::thread::spawn → stream_controller.restart_for_quality()
           └─ restart_output_stream()  (full DSP rebuild, unavoidable)
```

User-set quality (non-`Auto`) bypasses `managed_change` entirely; the gate is
written directly and the stream is restarted if the sample type differs.

---

## Snoop API

Short ring-buffer taps are placed throughout the signal graph:

| Method | What it returns |
|--------|----------------|
| `snapshot_output_snoop(key)` | latest samples from one string's output |
| `snapshot_all_output_snoops()` | all string outputs in one call |
| `snapshot_excitement_snoop(key)` | `(primary, secondary)` excitement pairs |
| `snapshot_all_excitement_snoops()` | all excitement pairs |
| `snapshot_input_snoop()` | mic input samples (empty when source = Entropy) |
| `snapshot_processed_output_spectrum()` | FFT of stereo output (left, right `BTreeMap<Hz → magnitude>`) |
| `poll_tuner_excitements()` | instantaneous excitement level per node |
| `poll_tuner_spectrum()` | latest FFT frame from the input analyser |

All snapshot methods call `Snoop::update()` to pull buffered samples off the DSP
thread before reading, so callers always receive the most recent available data.

[FunDSP]: https://github.com/SamiPerttu/fundsp
[CPAL]: https://github.com/RustAudio/cpal