use std::collections::VecDeque;
use std::iter;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::rt::telemetry;
use crate::rt::ProcessingMode;

use cpal::{OutputStreamTimestamp, StreamInstant};
use fundsp::buffer::BufferVec;
use fundsp::prelude::{AudioUnit, BigBlockAdapter, NetBackend};
use fundsp::thingbuf::ThingBuf;
use fundsp::{Frame, Size, MAX_BUFFER_SIZE};

const MAX_SILENCE_SECS: f64 = 0.5;

pub struct PlaybackCallbackConfig {
    pub input_buffer: Option<Arc<ThingBuf<f32>>>,
    pub sample_rate: u32,
    pub buffer_target_frames: usize,
    pub telemetry: Arc<ThingBuf<telemetry::Message>>,
}

// Decision logic types and helper
struct Decision {
    fill_size: usize,
    mode: ProcessingMode,
}

/// Telemetry-driven mode selection:
/// - Prefer Tick in steady state toward a steady buffer depth.
/// - Enter catch-up on underruns, negative slack EMA, or high estimated latency.
#[allow(clippy::too_many_arguments)]
fn decide(
    num_frames: usize,
    sample_rate: u32,
    remaining: usize,
    remaining_cap: usize,
    optimal_buffer_size: usize,
) -> Decision {
    todo!()
    // let steady_depth =
    //     ((buffer_target_frames as f64) * telemetry::STEADY_OCCUPANCY_RATIO).round() as usize;
    // let estimated_latency_ms =
    //     ((current_depth + net_latency_frames) as f64 / sample_rate as f64) * 1000.0;

    // let need_catch_up = local_underruns > 0
    //     || ema_compute_slack_ns < 0.0
    //     || estimated_latency_ms > buffer_duration_ms * telemetry::CATCH_UP_LATENCY_RATIO;

    // let deficit = steady_depth.saturating_sub(current_depth);
    // // Translate negative slack (behind schedule) into extra frames to produce
    // let extra = if ema_compute_slack_ns < 0.0 {
    //     let ns = (-ema_compute_slack_ns).max(0.0);
    //     ((ns / 1_000_000_000.0) * sample_rate as f64).round() as usize
    // } else {
    //     0
    // };

    // let fill_size = if need_catch_up {
    //     (deficit + extra).max(num_frames)
    // } else {
    //     deficit.max(num_frames)
    // };

    // let mode = if fill_size <= 4 {
    //     ProcessingMode::Tick
    // } else if fill_size <= 64 {
    //     ProcessingMode::Process
    // } else {
    //     ProcessingMode::ProcessBig
    // };

    // Decision { fill_size, mode }
}

pub fn playback_callback<const N: usize>(
    net: NetBackend,
    cfg: PlaybackCallbackConfig,
) -> Box<super::GenType> {
    let mut backend = BigBlockAdapter::new(Box::new(net));
    let net_latency = backend.latency();
    let input_buffer = cfg.input_buffer.clone();
    let sample_rate = cfg.sample_rate;
    let buffer_target_frames = cfg.buffer_target_frames;
    let telemetry_buff = cfg.telemetry.clone();

    // Target buffer derived from PlaybackQualityGate::buffer_size(...)
    let buffer_target_frames = buffer_target_frames;

    let mut frame_scratch: [f32; N] = [0_f32; N];

    let mut output_buffer = VecDeque::<[f32; N]>::with_capacity(buffer_target_frames);
    let mut frames_per_output_buffer = MAX_BUFFER_SIZE;

    // Fundsp small-batch buffers (64 samples max per channel), used in Mode::Process
    let mut process_in_buf = BufferVec::new(1);
    let mut process_out_buf = BufferVec::new(N);

    // Large batch buffers

    let mut process_in_big_buf: Vec<f32> = Vec::with_capacity(buffer_target_frames);
    let mut process_out_big_buf_inner: Vec<Vec<f32>> = Vec::with_capacity(N);
    process_out_big_buf_inner.fill_with(|| Vec::with_capacity(buffer_target_frames));

    // Warm-up: pre-fill to optimal (plus latency)
    {
        let warmup_fill = net_latency
            .map(|lat| lat.ceil() as usize + buffer_target_frames)
            .unwrap_or(buffer_target_frames);

        for _ in 0..warmup_fill {
            let input = input_buffer
                .as_ref()
                .and_then(|ib| ib.pop())
                .unwrap_or_default();

            backend.tick(&[input], &mut frame_scratch);
            output_buffer.push_back(frame_scratch);
        }
    }

    Box::new(
        move |ts: OutputStreamTimestamp, channels_frames: &mut [&mut [f32]]| {
            let now = Instant::now();
            let num_frames = channels_frames[0].len();

            for i in 0..num_frames {
                if let Some(frame) = output_buffer.pop_front() {
                    channels_frames.iter_mut().enumerate().for_each(|(ch, fr)| {
                        fr[i] = frame[ch];
                    });
                }
            }

            if num_frames > frames_per_output_buffer {
                output_buffer.reserve(num_frames - frames_per_output_buffer);
                frames_per_output_buffer = num_frames;
            }

            let remainig = output_buffer.len();
            let remainig_cap = output_buffer.capacity() - remainig;
            let Decision { fill_size, mode } = decide(
                frames_per_output_buffer,
                sample_rate,
                remainig,
                remainig_cap,
                buffer_target_frames,
            );

            match mode {
                ProcessingMode::Tick => {
                    for _ in 0..fill_size {
                        let input = input_buffer
                            .as_ref()
                            .and_then(|ib| ib.pop())
                            .unwrap_or_default();
                        backend.tick(&[input], &mut frame_scratch);
                        output_buffer.push_back(frame_scratch);
                    }
                }
                ProcessingMode::Process => {
                    let remainder = (fill_size % MAX_BUFFER_SIZE).min(1);
                    for _ in 0..(fill_size / MAX_BUFFER_SIZE + remainder).max(1) {
                        process_in_buf
                            .buffer_mut()
                            .channel_f32_mut(0)
                            .fill_with(|| {
                                input_buffer
                                    .as_ref()
                                    .and_then(|b| b.pop())
                                    .unwrap_or_default()
                            });

                        backend.process(
                            MAX_BUFFER_SIZE,
                            &process_in_buf.buffer_ref(),
                            &mut process_out_buf.buffer_mut(),
                        );
                        for i in 0..MAX_BUFFER_SIZE {
                            let mut it = 0..N;
                            frame_scratch
                                .fill_with(|| process_out_buf.at_f32(it.next().unwrap(), i));
                            output_buffer.push_back(frame_scratch);
                        }
                    }
                }
                ProcessingMode::ProcessBig => {
                    let mut process_out_big_buf: Vec<&mut [f32]> = Vec::from_iter(
                        process_out_big_buf_inner
                            .iter_mut()
                            .map(|v| v.split_at_mut(fill_size).0),
                    );
                    process_in_big_buf.iter_mut().take(fill_size).for_each(|s| {
                        *s = input_buffer
                            .as_ref()
                            .and_then(|b| b.pop())
                            .unwrap_or_default();
                    });
                    backend.process_big(
                        fill_size,
                        &[&process_in_big_buf[..fill_size]],
                        process_out_big_buf.as_mut_slice(),
                    );
                    for i in 0..fill_size {
                        output_buffer
                            .push_back(core::array::from_fn(|ch| process_out_big_buf[ch][i]));
                    }
                }
            }

            // // After catch-up production, reset accumulated latency
            // if accumulated_latency > sub_optimal_duration {
            //     accumulated_latency = Duration::ZERO;
            // }

            // // Telemetry: measure render, compute slack vs expected period and recompute latency
            // let computed_expected = Duration::from_secs_f64(num_frames as f64 / sample_rate as f64);
            // // Use timestamp hint only if not marked unreliable; otherwise rely on computed expected period.
            // let expected_period = if timestamp_unreliable {
            //     computed_expected
            // } else {
            //     // Currently, we prefer computed period; timestamp can be incorporated here if needed.
            //     // Keeping this branch for future enhancement where ts.playback pacing is trustworthy.
            //     computed_expected
            // };

            // // Telemetry: precise render timing and underrun note
            // let elapsed = start.elapsed();
            // if let Some(mut t) = telemetry.try_lock() {
            //     t.note_render(num_frames, elapsed);
            //     t.recompute_slack(expected_period);
            //     t.recompute_latency();
            //     t.note_underruns(local_underruns);
            // }

            // // Indicator equals current gate value (read-only in callback; engine writes on gate changes)
            // let _gate_val = quality.load(std::sync::atomic::Ordering::Relaxed);
        },
    ) as Box<super::GenType>
}
