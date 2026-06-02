use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;

use crate::quality::{PlaybackQualityGate, SampleType};
use crate::rt::telemetry;
use crate::rt::ProcessingMode;

use cpal::OutputStreamTimestamp;
use fundsp::buffer::BufferVec;
use fundsp::prelude::{AudioUnit, BigBlockAdapter, NetBackend};
use fundsp::setting::TrySendError;
use fundsp::thingbuf::ThingBuf;
use fundsp::MAX_BUFFER_SIZE;
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
    const PROCESS_BIG_DELTA_THRESHOLD_I: isize = (MAX_BUFFER_SIZE as isize) * 2;

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
        // plenty of capacity, moderate deficit, callback somewhat larger than optimal
        // still prefer small-batch processing to avoid queue overshoot.
        (_, MIN_BUFFER_SIZE_I..MAX_BUFFER_SIZE_I, 1..PROCESS_BIG_DELTA_THRESHOLD_I) => Decision {
            fill_size: deficit.unsigned_abs(),
            mode: ProcessingMode::Process,
        },
        // plenty of capacity, moderate deficit, callback much larger than optimal — go big
        (_, MIN_BUFFER_SIZE_I..MAX_BUFFER_SIZE_I, PROCESS_BIG_DELTA_THRESHOLD_I..) => Decision {
            fill_size: deficit.unsigned_abs(),
            mode: ProcessingMode::ProcessBig,
        },
        // plenty of capacity, large deficit, callback no larger than optimal
        // keep batching small unless we're clearly in catch-up.
        (_, MAX_BUFFER_SIZE_I.., ..=0) => Decision {
            fill_size: deficit.unsigned_abs(),
            mode: ProcessingMode::Process,
        },
        // plenty of capacity, large deficit, callback larger than optimal — go big for throughput.
        (_, MAX_BUFFER_SIZE_I.., 1..MAX_BUFFER_SIZE_I) => Decision {
            fill_size: deficit.unsigned_abs(),
            mode: ProcessingMode::ProcessBig,
        },
    }
}

const BUFFER_CAP_MS: f64 = 50_f64;

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
    let mut process_in_big_buf: Vec<f32> = vec![0.0_f32; init_cap];
    let mut process_out_big_buf_inner: Vec<Vec<f32>> =
        (0..N).map(|_| vec![0.0_f32; init_cap]).collect();

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
            // Re-read the target from the quality gate so buffer-size changes take
            // effect dynamically without a stream restart.
            let buffer_target_frames = cfg.quality.read().buffer_size(None) as usize;

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
                if process_in_big_buf.len() < updated_cap {
                    process_in_big_buf.resize(updated_cap, 0.0_f32);
                }
                process_out_big_buf_inner.iter_mut().for_each(|inner| {
                    if inner.len() < updated_cap {
                        inner.resize(updated_cap, 0.0_f32);
                    }
                });
                frames_per_output_buffer = num_frames;
            }

            let remaining_filled = output_buffer.len();
            let remaining_cap = output_buffer.capacity() - remaining_filled;
            let Decision { fill_size, mode } = decide(
                frames_per_output_buffer as isize,
                buffer_target_frames as isize,
                remaining_filled as isize,
                remaining_cap as isize,
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
                    let mut remaining = fill_size;

                    while remaining > 0 {
                        let chunk = remaining.min(MAX_BUFFER_SIZE);

                        {
                            let input_channel = process_in_buf.buffer_mut().channel_f32_mut(0);
                            for sample in input_channel.iter_mut().take(chunk) {
                                *sample = input_buffer
                                    .as_ref()
                                    .and_then(|b| b.pop())
                                    .unwrap_or_default();
                            }
                        }

                        backend.process(
                            chunk,
                            &process_in_buf.buffer_ref(),
                            &mut process_out_buf.buffer_mut(),
                        );

                        for i in 0..chunk {
                            let mut it = 0..N;
                            frame_scratch
                                .fill_with(|| process_out_buf.at_f32(it.next().unwrap(), i));
                            output_buffer.push_back(frame_scratch);
                        }

                        remaining -= chunk;
                    }
                }
                ProcessingMode::ProcessBig => {
                    // Ensure inner output vecs are long enough for this fill_size.
                    // This handles the first callback before any capacity-update path runs,
                    // and any edge case where fill_size exceeds the previously allocated length.
                    process_out_big_buf_inner.iter_mut().for_each(|v| {
                        if v.len() < fill_size {
                            v.resize(fill_size, 0.0_f32);
                        }
                    });
                    if process_in_big_buf.len() < fill_size {
                        process_in_big_buf.resize(fill_size, 0.0_f32);
                    }
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
            let queue_depth_frames_after_fill = output_buffer.len();
            let produced_frames = queue_depth_frames_after_fill.saturating_sub(remaining_filled);

            let summary = telemetry::Message {
                mode,
                filled_size: fill_size,
                buffer_size: num_frames,
                queue_depth_frames_before_fill: remaining_filled,
                queue_depth_frames_after_fill,
                target_buffer_frames: buffer_target_frames,
                produced_frames,
                callback_started_at: start_ts,
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
            ) && !TELEMETRY_CHANNEL_CLOSED.swap(true, Ordering::Relaxed)
            {
                log::warn!("Telemetry channel was closed; disabling telemetry updates");
            }
        },
    ) as Box<super::GenType>
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use cpal::StreamInstant;
    use fundsp::prelude::{pass, split, Net, U2};

    #[test]
    fn decide_capacity_exceeded_returns_none() {
        let decision = decide(64, 64, 64, 0);
        assert_eq!(decision.mode, ProcessingMode::None);
        assert_eq!(decision.fill_size, 0);
    }

    #[test]
    fn decide_small_remaining_capacity_uses_tick() {
        let decision = decide(64, 64, 64, 5);
        assert_eq!(decision.mode, ProcessingMode::Tick);
        assert_eq!(decision.fill_size, 5);
    }

    #[test]
    fn decide_large_negative_deficit_uses_single_tick() {
        let decision = decide(64, 64, 200, 100);
        assert_eq!(decision.mode, ProcessingMode::Tick);
        assert_eq!(decision.fill_size, 1);
    }

    #[test]
    fn decide_small_negative_deficit_backfills_with_tick() {
        let decision = decide(64, 64, 100, 100);
        assert_eq!(decision.mode, ProcessingMode::Tick);
        assert_eq!(decision.fill_size, 28);
    }

    #[test]
    fn decide_large_deficit_and_large_delta_uses_process_big() {
        let decision = decide(128, 64, 0, 100);
        assert_eq!(decision.mode, ProcessingMode::ProcessBig);
        assert_eq!(decision.fill_size, 64);
    }

    #[test]
    fn decide_moderate_deficit_and_small_positive_delta_uses_process() {
        let decision = decide(96, 64, 32, 100);
        assert_eq!(decision.mode, ProcessingMode::Process);
        assert_eq!(decision.fill_size, 32);
    }

    #[test]
    fn decide_slight_deficit_uses_tick() {
        let decision = decide(64, 64, 60, 100);
        assert_eq!(decision.mode, ProcessingMode::Tick);
        assert_eq!(decision.fill_size, 4);
    }

    #[test]
    fn decide_moderate_deficit_with_non_positive_delta_uses_process() {
        let decision = decide(32, 64, 40, 100);
        assert_eq!(decision.mode, ProcessingMode::Process);
        assert_eq!(decision.fill_size, 24);
    }

    #[test]
    fn decide_moderate_deficit_with_mid_positive_delta_uses_process() {
        let decision = decide(128, 64, 24, 100);
        assert_eq!(decision.mode, ProcessingMode::Process);
        assert_eq!(decision.fill_size, 40);
    }

    #[test]
    fn decide_moderate_deficit_with_very_large_delta_uses_process_big() {
        let decision = decide(192, 64, 24, 100);
        assert_eq!(decision.mode, ProcessingMode::ProcessBig);
        assert_eq!(decision.fill_size, 40);
    }

    #[test]
    fn decide_large_deficit_with_non_positive_delta_uses_process() {
        let decision = decide(32, 64, 0, 100);
        assert_eq!(decision.mode, ProcessingMode::Process);
        assert_eq!(decision.fill_size, 64);
    }

    #[test]
    fn decide_large_deficit_with_small_positive_delta_uses_process_big() {
        let decision = decide(96, 64, 0, 100);
        assert_eq!(decision.mode, ProcessingMode::ProcessBig);
        assert_eq!(decision.fill_size, 64);
    }

    fn test_backend() -> NetBackend {
        let mut net = Net::new(1, 2);
        let passthrough_id = net.push(Box::new(pass() >> split::<U2>()));
        net.connect_input(0, passthrough_id, 0);
        net.connect_output(passthrough_id, 0, 0);
        net.connect_output(passthrough_id, 1, 1);
        net.set_sample_rate(44_100.0);
        net.check();
        net.backend()
    }

    fn output_ts() -> OutputStreamTimestamp {
        let callback = StreamInstant::new(1, 0);
        let playback = callback
            .add(Duration::from_millis(2))
            .expect("timestamp add should not overflow");

        OutputStreamTimestamp { callback, playback }
    }

    fn invoke_callback(
        callback: &mut Box<crate::rt::stream::GenType>,
        frames: usize,
    ) -> (Vec<f32>, Vec<f32>) {
        let mut left = vec![0.0_f32; frames];
        let mut right = vec![0.0_f32; frames];
        let mut channels = [left.as_mut_slice(), right.as_mut_slice()];
        callback(output_ts(), &mut channels);
        (left, right)
    }

    fn test_cfg(
        quality: Arc<RwLock<PlaybackQualityGate>>,
        telemetry: telemetry::TelemetrySender,
    ) -> PlaybackCallbackConfig {
        PlaybackCallbackConfig {
            input_buffer: None,
            sample_rate: 44_100,
            buffer_target_frames: PlaybackQualityGate::Medium.buffer_size(None) as usize,
            sample_type: SampleType::F32,
            quality,
            telemetry,
        }
    }

    #[test]
    fn playback_callback_emits_telemetry_message() {
        TELEMETRY_CHANNEL_CLOSED.store(false, Ordering::Relaxed);
        let (telemetry_tx, telemetry_rx) = telemetry::create_telemetry_channel();
        let quality = Arc::new(RwLock::new(PlaybackQualityGate::Medium));
        let mut callback = playback_callback::<2>(test_backend(), test_cfg(quality, telemetry_tx));

        let (_left, _right) = invoke_callback(&mut callback, 128);
        let msg = telemetry_rx
            .try_recv()
            .expect("telemetry message should be available after callback");

        assert_eq!(msg.buffer_size, 128);
        assert_eq!(msg.sample_type, SampleType::F32);
        assert_eq!(msg.quality, PlaybackQualityGate::Medium);
    }

    #[test]
    fn playback_callback_reacts_to_quality_changes() {
        TELEMETRY_CHANNEL_CLOSED.store(false, Ordering::Relaxed);
        let (telemetry_tx, telemetry_rx) = telemetry::create_telemetry_channel();
        let quality = Arc::new(RwLock::new(PlaybackQualityGate::Medium));
        let mut callback =
            playback_callback::<2>(test_backend(), test_cfg(quality.clone(), telemetry_tx));

        let (_left, _right) = invoke_callback(&mut callback, 96);
        let _ = telemetry_rx.try_recv();

        *quality.write() = PlaybackQualityGate::LoFi;
        let (_left2, _right2) = invoke_callback(&mut callback, 96);
        let msg = telemetry_rx
            .try_recv()
            .expect("telemetry should reflect updated quality gate");

        assert_eq!(
            msg.target_buffer_frames,
            PlaybackQualityGate::LoFi.buffer_size(None) as usize
        );
        assert_eq!(msg.quality, PlaybackQualityGate::LoFi);
    }

    #[test]
    fn playback_callback_handles_larger_callback_buffer_resize_path() {
        TELEMETRY_CHANNEL_CLOSED.store(false, Ordering::Relaxed);
        let (telemetry_tx, telemetry_rx) = telemetry::create_telemetry_channel();
        let quality = Arc::new(RwLock::new(PlaybackQualityGate::Medium));
        let mut callback = playback_callback::<2>(test_backend(), test_cfg(quality, telemetry_tx));

        let (_left, _right) = invoke_callback(&mut callback, 64);
        let _ = telemetry_rx.try_recv();

        let (_left2, _right2) = invoke_callback(&mut callback, MAX_BUFFER_SIZE * 3);
        let msg = telemetry_rx
            .try_recv()
            .expect("telemetry should be emitted for resized callback");

        assert_eq!(msg.buffer_size, MAX_BUFFER_SIZE * 3);
    }

    #[test]
    fn playback_callback_tolerates_closed_telemetry_channel() {
        TELEMETRY_CHANNEL_CLOSED.store(false, Ordering::Relaxed);
        let (telemetry_tx, telemetry_rx) = telemetry::create_telemetry_channel();
        drop(telemetry_rx);

        let quality = Arc::new(RwLock::new(PlaybackQualityGate::Medium));
        let mut callback = playback_callback::<2>(test_backend(), test_cfg(quality, telemetry_tx));

        let (_left, _right) = invoke_callback(&mut callback, 64);
        let (_left2, _right2) = invoke_callback(&mut callback, 64);

        assert!(TELEMETRY_CHANNEL_CLOSED.load(Ordering::Relaxed));
        TELEMETRY_CHANNEL_CLOSED.store(false, Ordering::Relaxed);
    }
}
