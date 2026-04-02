use std::collections::VecDeque;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

use crate::quality::{PlaybackQualityGate, SampleType};
use crate::rt::ProcessingMode;
use crate::rt::telemetry;

use cpal::OutputStreamTimestamp;
use fundsp::MAX_BUFFER_SIZE;
use fundsp::buffer::BufferVec;
use fundsp::prelude::{AudioUnit, BigBlockAdapter, NetBackend};
use fundsp::setting::TrySendError;
use fundsp::thingbuf::ThingBuf;
use parking_lot::RwLock;

pub struct PlaybackCallbackConfig {
    pub input_buffer: Option<Arc<ThingBuf<f32>>>,
    pub sample_rate: u32,
    pub buffer_target_frames: usize,
    pub sample_type: SampleType,
    pub quality: Arc<RwLock<PlaybackQualityGate>>,
    pub telemetry: telemetry::TelemetrySender,
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
    cb_buffer_size: isize,
    optimal_buffer_size: isize,
    remaining_filled: isize,
    remaining_cap: isize,
) -> Decision {
    const MAX_BUFFER_SIZE_I: isize = MAX_BUFFER_SIZE as isize;
    const MIN_BUFFER_SIZE_I: isize = 8;

    // -y — suboptimal (smaller) callback buffer size
    // 0 — optimal callback buffer size
    // +y — suboptimal (larger) callback buffer size
    let buffers_delta = cb_buffer_size - optimal_buffer_size;
    // -x — excess frames
    // 0 — no deficit
    // +x — deficit frames
    let deficit = (optimal_buffer_size - remaining_filled).min(remaining_cap);

    match (remaining_cap, deficit, buffers_delta) {
        // capacity exceeded
        (..=0, _, _) => Decision {
            fill_size: 0,
            mode: ProcessingMode::None,
        },
        // some capacity remains...
        (1..=MIN_BUFFER_SIZE_I, _, _) => Decision {
            fill_size: remaining_cap.unsigned_abs(),
            mode: ProcessingMode::Tick,
        },
        // excess frames beyond cb size
        (_, d, _) if d < 0 && d.abs() >= cb_buffer_size => Decision {
            fill_size: 1,
            mode: ProcessingMode::Tick,
        },
        // excess frames, but less than next tick
        (_, d, _) if d < 0 && d.abs() <= cb_buffer_size => Decision {
            fill_size: (cb_buffer_size - d.abs()).unsigned_abs(),
            mode: ProcessingMode::Tick,
        },
        // excessive buffer size, large deficit
        (_, MAX_BUFFER_SIZE_I.., MAX_BUFFER_SIZE_I..) => Decision {
            fill_size: deficit.unsigned_abs(),
            mode: ProcessingMode::ProcessBig,
        },
        // slightly excessive buffer, deficit frames
        (_, MIN_BUFFER_SIZE_I..MAX_BUFFER_SIZE_I, 1..MAX_BUFFER_SIZE_I) => Decision {
            fill_size: deficit.unsigned_abs(),
            mode: ProcessingMode::Process,
        },
        // slight deficit
        (_, ..MIN_BUFFER_SIZE_I, _) => Decision {
            fill_size: deficit.unsigned_abs(),
            mode: ProcessingMode::Tick,
        },
        // plenty of capacity, moderate deficit, buffer optimal or smaller — small-batch process
        (_, MIN_BUFFER_SIZE_I..MAX_BUFFER_SIZE_I, ..=0) => Decision {
            fill_size: deficit.unsigned_abs(),
            mode: ProcessingMode::Process,
        },
        // plenty of capacity, moderate deficit, buffer much larger than optimal — go big
        (_, MIN_BUFFER_SIZE_I..MAX_BUFFER_SIZE_I, 64..) => Decision {
            fill_size: deficit.unsigned_abs(),
            mode: ProcessingMode::ProcessBig,
        },
        // plenty of capacity, large deficit, buffer within moderate excess — go big for throughput
        (_, MAX_BUFFER_SIZE_I.., ..MAX_BUFFER_SIZE_I) => Decision {
            fill_size: deficit.unsigned_abs(),
            mode: ProcessingMode::ProcessBig,
        },
    }
}

const BUFFER_CAP_MS: f64 = 75_f64;

static TELEMETRY_CHANNEL_CLOSED: AtomicBool = AtomicBool::new(false);

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

    let compute_cap = |sample_rate: u32,
                       buffer_target_frames: usize,
                       actual_buffer_size: Option<usize>|
     -> usize {
        // Convert BUFFER_CAP_MS to frames: frames = ms * sample_rate / 1000
        let max_cap_frames: usize = (BUFFER_CAP_MS * (sample_rate as f64) / 1000.0).ceil() as usize;

        // Use actual buffer size if provided; otherwise fall back to MAX_BUFFER_SIZE
        let block = actual_buffer_size.unwrap_or(MAX_BUFFER_SIZE).max(1);

        // Round target up to whole blocks
        let target_rounded_up = buffer_target_frames.div_ceil(block).max(1) * block;

        // Enforce cap: round cap down to whole blocks
        let cap_rounded_down = (max_cap_frames / block).max(1) * block;

        // Final capacity is the minimum of rounded target and rounded cap
        target_rounded_up.min(cap_rounded_down)
    };

    // Target buffer derived from PlaybackQualityGate::buffer_size(...)
    let mut frames_per_output_buffer = MAX_BUFFER_SIZE;
    let init_cap = compute_cap(sample_rate, buffer_target_frames, None);

    let mut output_buffer = VecDeque::<[f32; N]>::with_capacity(init_cap);

    // tick buffer
    let mut frame_scratch: [f32; N] = [0_f32; N];

    // Fundsp small-batch buffers (64 samples max per channel), used in Mode::Process
    let mut process_in_buf = BufferVec::new(1);
    let mut process_out_buf = BufferVec::new(N);

    // Large batch buffers
    let mut process_in_big_buf: Vec<f32> = Vec::with_capacity(init_cap);
    let mut process_out_big_buf_inner: Vec<Vec<f32>> = Vec::with_capacity(N);
    process_out_big_buf_inner.fill_with(|| Vec::with_capacity(init_cap));

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
            let start_ts = Instant::now();
            let num_frames = channels_frames[0].len();

            for i in 0..num_frames {
                if let Some(frame) = output_buffer.pop_front() {
                    channels_frames.iter_mut().enumerate().for_each(|(ch, fr)| {
                        fr[i] = frame[ch];
                    });
                }
            }

            if num_frames > frames_per_output_buffer {
                let updated_cap = compute_cap(sample_rate, buffer_target_frames, Some(num_frames));

                if let Some(add) = updated_cap.checked_sub(output_buffer.capacity()) {
                    output_buffer.reserve(add);
                }
                if let Some(add) = updated_cap.checked_sub(process_in_big_buf.capacity()) {
                    process_in_big_buf.reserve(add);
                }
                process_out_big_buf_inner.iter_mut().for_each(|inner| {
                    if let Some(add) = updated_cap.checked_sub(inner.capacity()) {
                        inner.reserve(add);
                    }
                });
                frames_per_output_buffer = num_frames;
            }

            let remainig_filled = output_buffer.len();
            let remainig_cap = output_buffer.capacity() - remainig_filled;
            let Decision { fill_size, mode } = decide(
                frames_per_output_buffer as isize,
                buffer_target_frames as isize,
                remainig_filled as isize,
                remainig_cap as isize,
            );

            match mode {
                ProcessingMode::None => {}
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
                    #[allow(clippy::needless_range_loop)]
                    for i in 0..fill_size {
                        output_buffer
                            .push_back(core::array::from_fn(|ch| process_out_big_buf[ch][i]));
                    }
                }
            }

            let processing_time = Instant::now().duration_since(start_ts);

            let summary = telemetry::Message {
                mode,
                filled_size: fill_size,
                buffer_size: num_frames,
                processing_time,
                estimated_latency: ts
                    .playback
                    .duration_since(&ts.callback)
                    .and_then(|d| processing_time.checked_sub(d)),
                quality: (*cfg.quality.read()),
                sample_type: cfg.sample_type,
            };

            if matches!(
                telemetry_buff.try_send(summary),
                Err(TrySendError::Closed(_))
            ) {
                if !TELEMETRY_CHANNEL_CLOSED.swap(true, Ordering::Relaxed) {
                    log::warn!("Telemetry channel was closed; disabling telemetry updates");
                }
            }
        },
    ) as Box<super::GenType>
}
