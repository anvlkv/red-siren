/*!
Intro (splash) DSP visualization events.

This module defines the batched snoop event emitted by the backend intro
fundsp engine. The engine collects 11 low-frequency “snoops” (virtual
oscilloscope taps) and sends them together in a single batch message
to minimize IPC overhead.

Design:
- Exactly one channel event per tick (e.g. ~20 FPS).
- Each snoop has the same underlying base oscillator set but a different
  modulation depth (linear ramp 0.0 -> 0.7 across snoop_id 1..=11).
- Samples are already normalized/clamped to roughly -1.0..1.0 in the
  backend prior to serialization (frontend should not mutate them).

Payload Semantics:
- `t_unix_ms`: Wall-clock timestamp (milliseconds since Unix epoch) when
  the batch was assembled. Useful for latency metrics or drift detection.
- `snoops`: Ordered collection; each element carries:
    * `snoop_id`          : 1-based identifier (1..=11)
    * `modulation_depth`  : Depth applied in backend (0.0..=0.7)
    * `samples`           : Time-domain window (latest buffer)

Future Extension:
- Add optional spectral summary (RMS, peak) per snoop if needed.
- Add engine status or performance diagnostics.

MAYA DRY KISS: Keep this file lean and focused on data definition only.
*/

use serde::{Deserialize, Serialize};

/// Event name for a full batch of intro snoop samples.
/// Emitted over a Tauri Channel as JSON.
pub const INTRO_SNOOP_BATCH: &str = "intro_snoop_batch";

/// One snoop’s current snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntroSnoopSample {
    /// 1-based snoop identifier (1..=11).
    pub snoop_id: u8,
    /// Modulation depth applied to this snoop (0.0..=0.7).
    pub modulation_depth: f32,
    /// Most recent time-domain samples (normalized -1.0..1.0).
    pub samples: Vec<f32>,
}

/// Batched payload containing all snoops for a single engine tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntroSnoopBatchPayload {
    /// Unix timestamp (ms) when the batch was produced.
    pub t_unix_ms: u64,
    /// Collection of snoop snapshots. Typically length == 11.
    pub snoops: Vec<IntroSnoopSample>,
}
