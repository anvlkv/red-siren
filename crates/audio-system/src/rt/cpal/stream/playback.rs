use std::collections::VecDeque;
use std::iter;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::telemetry;
use cpal::OutputStreamTimestamp;
use fundsp::buffer::BufferVec;
use fundsp::prelude::{AudioUnit, BigBlockAdapter, NetBackend};
use fundsp::thingbuf::ThingBuf;

const MAX_SILENCE_SECS: f64 = 0.5;

pub struct PlaybackCallbackConfig {
    pub input_buffer: Option<Arc<ThingBuf<f32>>>,
    pub quality: Arc<std::sync::atomic::AtomicI8>,
    pub no_reset_on_silence: Arc<parking_lot::RwLock<bool>>,
    pub sample_rate: u32,
    pub buffer_target_frames: usize,
    pub telemetry: Arc<parking_lot::Mutex<telemetry::PlaybackTelemetry>>,
}

// Decision logic types and helper
struct Decision {
    fill_size: usize,
    mode: Mode,
}

enum Mode {
    Tick,
    Process,
    ProcessBig,
}

/// Telemetry-driven mode selection:
/// - Prefer Tick in steady state toward a steady buffer depth.
/// - Enter catch-up on underruns, negative slack EMA, or high estimated latency.
#[allow(clippy::too_many_arguments)]
fn decide(
    num_frames: usize,
    sample_rate: u32,
    buffer_target_frames: usize,
    current_depth: usize,
    net_latency_frames: usize,
    ema_compute_slack_ns: f64,
    local_underruns: u32,
    buffer_duration_ms: f64,
) -> Decision {
    let steady_depth =
        ((buffer_target_frames as f64) * telemetry::STEADY_OCCUPANCY_RATIO).round() as usize;
    let estimated_latency_ms =
        ((current_depth + net_latency_frames) as f64 / sample_rate as f64) * 1000.0;

    let need_catch_up = local_underruns > 0
        || ema_compute_slack_ns < 0.0
        || estimated_latency_ms > buffer_duration_ms * telemetry::CATCH_UP_LATENCY_RATIO;

    let deficit = steady_depth.saturating_sub(current_depth);
    // Translate negative slack (behind schedule) into extra frames to produce
    let extra = if ema_compute_slack_ns < 0.0 {
        let ns = (-ema_compute_slack_ns).max(0.0);
        ((ns / 1_000_000_000.0) * sample_rate as f64).round() as usize
    } else {
        0
    };

    let fill_size = if need_catch_up {
        (deficit + extra).max(num_frames)
    } else {
        deficit.max(num_frames)
    };

    let mode = if fill_size <= 4 {
        Mode::Tick
    } else if fill_size <= 64 {
        Mode::Process
    } else {
        Mode::ProcessBig
    };

    Decision { fill_size, mode }
}

pub fn playback_callback(net: NetBackend, cfg: PlaybackCallbackConfig) -> Box<super::GenType> {
    let mut backend = BigBlockAdapter::new(Box::new(net));
    let net_latency = backend.latency();
    let input_buffer = cfg.input_buffer.clone();
    let quality = cfg.quality.clone();
    let no_reset_on_silence = cfg.no_reset_on_silence.clone();
    let sample_rate = cfg.sample_rate;
    let buffer_target_frames = cfg.buffer_target_frames;
    let telemetry = cfg.telemetry.clone();

    // Target buffer derived from PlaybackQualityGate::buffer_size(...)
    let optimal_cap = buffer_target_frames;
    let buffer_duration_ms = (optimal_cap as f64 / sample_rate as f64) * 1000.0;
    let sub_optimal_duration = Duration::from_secs_f64(optimal_cap as f64 / sample_rate as f64);
    let sample_duration = Duration::from_secs_f64(1.0 / sample_rate as f64);

    let mut lr_frame_scratch = [0.0; 2];
    let mut i_batch_scratch = vec![0.0_f32; 512];
    let mut l_batch_scratch = vec![0.0_f32; 512];
    let mut r_batch_scratch = vec![0.0_f32; 512];
    let mut output_buffer = VecDeque::<(f32, f32)>::with_capacity(optimal_cap);
    let mut frames_per_output_buffer = 64_usize;

    // Fundsp small-batch buffers (64 samples max per channel), used in Mode::Process
    let mut process_in_buf = BufferVec::new(1);
    let mut process_out_buf = BufferVec::new(2);

    // State used to decide catch-up
    let mut accumulated_latency = Duration::ZERO;
    let mut accumulated_silence = Duration::ZERO;

    // Warm-up: pre-fill to optimal (plus latency), then clear to align timing
    {
        let warmup_fill = net_latency
            .map(|lat| lat.ceil() as usize + optimal_cap)
            .unwrap_or(optimal_cap);

        for _ in 0..warmup_fill {
            let input = input_buffer
                .as_ref()
                .and_then(|ib| ib.pop())
                .unwrap_or_default();
            backend.tick(&[input], &mut lr_frame_scratch);
            output_buffer.push_back((lr_frame_scratch[0], lr_frame_scratch[1]));
        }

        output_buffer.clear();
    }

    // Timestamp reliability guard state (Phase A):
    // If drift beyond buffer duration or repeated underruns are observed, mark timestamp as unreliable
    // and rely on computed expected periods for slack/latency decisions.
    let mut timestamp_unreliable = false;
    let mut drift_strikes: u32 = 0;
    const DRIFT_STRIKES_LIMIT: u32 = 3;

    Box::new(
        move |_ts: OutputStreamTimestamp, frames: &mut [&mut [f32]]| {
            // quality indicator handling is engine-owned; callback remains read-only
            let (l_frames, r_frames) = frames.split_at_mut(1);
            let num_frames = l_frames[0].len();
            if num_frames != frames_per_output_buffer {
                frames_per_output_buffer = num_frames;
            }

            // Telemetry: update queue depth and sample rate for this callback
            if let Some(mut t) = telemetry.try_lock() {
                t.sample_rate = sample_rate;
                t.last_callback_frames = num_frames;
                t.queue_depth_frames = output_buffer.len();
                t.net_latency_frames = net_latency.map(|d| d.ceil() as usize).unwrap_or(0);
            }

            // Write out available frames, count underruns
            let mut local_underruns = 0u32;
            for (l, r) in l_frames[0].iter_mut().zip(r_frames[0].iter_mut()) {
                let frame = match output_buffer.pop_front() {
                    Some(f) => f,
                    None => {
                        // Underrun: silence and account drift
                        *l = 0.0;
                        *r = 0.0;
                        local_underruns = local_underruns.saturating_add(1);
                        (0.0, 0.0)
                    }
                };

                if frame.0.is_nan()
                    || frame.1.is_nan()
                    || frame.0.is_infinite()
                    || frame.1.is_infinite()
                {
                    // silently clamp to 0 without logging
                    *l = 0.0;
                    *r = 0.0;
                } else {
                    *l = frame.0;
                    *r = frame.1;
                }
            }

            if !*no_reset_on_silence.read()
                && l_frames[0]
                    .iter()
                    .zip(r_frames[0].iter())
                    .all(|(l, r)| !l.is_normal() && !r.is_normal())
            {
                accumulated_silence +=
                    Duration::from_secs_f64(num_frames as f64 * sample_duration.as_secs_f64());
            }

            if accumulated_silence >= Duration::from_secs_f64(MAX_SILENCE_SECS) {
                log::warn!("Resetting network. Accumulated silence: {accumulated_silence:?}");
                backend.reset();
                accumulated_silence = Duration::ZERO;
                // quality indicator: resetting state noted; engine will reflect via atomic
            }

            // Update latency from this callback's underruns
            if local_underruns > 0 {
                let added =
                    Duration::from_secs_f64(local_underruns as f64 * sample_duration.as_secs_f64());
                accumulated_latency = accumulated_latency.saturating_add(added);
                // quality indicator: underruns noted; engine will reflect via atomic
            }

            // Decide fill and mode using telemetry-driven logic
            let start = Instant::now();
            let current_len = output_buffer.len();
            let remaining_capacity = optimal_cap.saturating_sub(current_len);

            // Timestamp reliability guard evaluation (simple heuristic):
            // If we see underruns or estimated latency exceeding buffer duration repeatedly,
            // consider the device timestamp pacing unreliable and favor computed expected periods.
            let estimated_latency_ms_snapshot = {
                if let Some(t) = telemetry.try_lock() {
                    ((current_len + t.net_latency_frames) as f64 / sample_rate as f64) * 1000.0
                } else {
                    ((current_len) as f64 / sample_rate as f64) * 1000.0
                }
            };
            if local_underruns > 0 || estimated_latency_ms_snapshot > buffer_duration_ms {
                drift_strikes = drift_strikes.saturating_add(1);
                if drift_strikes >= DRIFT_STRIKES_LIMIT {
                    timestamp_unreliable = true;
                }
            } else if drift_strikes > 0 {
                // Cool off strikes gradually
                drift_strikes = drift_strikes.saturating_sub(1);
            }

            // Read telemetry snapshot needed for decision
            let (ema_slack_ns, net_latency_frames) = {
                if let Some(t) = telemetry.try_lock() {
                    (t.ema_compute_slack_ns, t.net_latency_frames)
                } else {
                    (0.0, 0)
                }
            };
            let Decision { fill_size, mode } = decide(
                num_frames,
                sample_rate,
                optimal_cap,
                current_len,
                net_latency_frames,
                ema_slack_ns,
                local_underruns,
                buffer_duration_ms,
            );

            // Ensure we don't exceed remaining capacity and at least produce num_frames
            let fill_size = fill_size.min(remaining_capacity).max(num_frames);

            // Prefer tick; use batch modes only to catch up
            match mode {
                Mode::Tick => {
                    for _ in 0..fill_size {
                        let input = input_buffer
                            .as_ref()
                            .and_then(|ib| ib.pop())
                            .unwrap_or_default();
                        backend.tick(&[input], &mut lr_frame_scratch);
                        output_buffer.push_back((lr_frame_scratch[0], lr_frame_scratch[1]));
                    }
                }
                Mode::Process => {
                    // Ensure we have <= 64 samples (fundsp BufferVec is fixed-length 64)
                    let count = fill_size.min(64);
                    // Fill input scalar channel
                    if let Some(ib) = input_buffer.as_ref() {
                        let chan = process_in_buf.buffer_mut().channel_f32_mut(0);
                        for v in chan.iter_mut().take(count) {
                            *v = ib.pop().unwrap_or(0.0);
                        }
                        // Zero the remainder if any
                        for v in chan.iter_mut().skip(count) {
                            *v = 0.0;
                        }
                    } else {
                        // No input buffer; zero channel
                        let chan = process_in_buf.buffer_mut().channel_f32_mut(0);
                        chan.iter_mut().for_each(|v| *v = 0.0);
                    }

                    // Clear output buffer before processing
                    process_out_buf.clear();

                    // Call fundsp small-batch process with BufferRef/BufferMut
                    backend.process(
                        count,
                        &process_in_buf.buffer_ref(),
                        &mut process_out_buf.buffer_mut(),
                    );

                    // Extend our output queue with processed samples
                    let l_out = process_out_buf.buffer_ref().channel_f32(0);
                    let r_out = process_out_buf.buffer_ref().channel_f32(1);
                    output_buffer.extend(
                        l_out
                            .iter()
                            .zip(r_out.iter())
                            .take(count)
                            .map(|(l, r)| (*l, *r)),
                    );
                }
                Mode::ProcessBig => {
                    if fill_size > l_batch_scratch.len() {
                        l_batch_scratch.resize(fill_size, 0.0);
                        r_batch_scratch.resize(fill_size, 0.0);
                        i_batch_scratch.resize(fill_size, 0.0);
                    }
                    if let Some(ib) = input_buffer.as_ref() {
                        i_batch_scratch
                            .iter_mut()
                            .take(fill_size)
                            .zip(iter::from_fn(|| ib.pop()))
                            .for_each(|(v, s)| *v = s);
                    }
                    backend.process_big(
                        fill_size,
                        &[&i_batch_scratch[..fill_size]],
                        &mut [
                            &mut l_batch_scratch[..fill_size],
                            &mut r_batch_scratch[..fill_size],
                        ],
                    );
                    output_buffer
                        .extend((0..fill_size).map(|i| (l_batch_scratch[i], r_batch_scratch[i])));
                }
            }

            // After catch-up production, reset accumulated latency
            if accumulated_latency > sub_optimal_duration {
                accumulated_latency = Duration::ZERO;
            }

            // Telemetry: measure render, compute slack vs expected period and recompute latency
            let computed_expected = Duration::from_secs_f64(num_frames as f64 / sample_rate as f64);
            // Use timestamp hint only if not marked unreliable; otherwise rely on computed expected period.
            let expected_period = if timestamp_unreliable {
                computed_expected
            } else {
                // Currently, we prefer computed period; timestamp can be incorporated here if needed.
                // Keeping this branch for future enhancement where ts.playback pacing is trustworthy.
                computed_expected
            };

            // Telemetry: precise render timing and underrun note
            let elapsed = start.elapsed();
            if let Some(mut t) = telemetry.try_lock() {
                t.note_render(num_frames, elapsed);
                t.recompute_slack(expected_period);
                t.recompute_latency();
                t.note_underruns(local_underruns);
            }

            // Indicator equals current gate value (read-only in callback; engine writes on gate changes)
            let _gate_val = quality.load(std::sync::atomic::Ordering::Relaxed);
        },
    ) as Box<super::GenType>
}
