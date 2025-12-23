use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use cpal::OutputStreamTimestamp;
use fundsp::hacker::{AudioUnit, BigBlockAdapter, NetBackend};
use fundsp::thingbuf::ThingBuf;
use parking_lot::RwLock;

use crate::util::S;

const BASE_OPTIMAL_BUFFER_MILLIS: f64 = 80.0;
const BASE_MAX_BUFFER_MILLIS: f64 = 320.0;
const HIGH_WATER_RATIO: f32 = 0.9;
const LOW_WATER_RATIO: f32 = 0.2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Processing speed / quality optimization
enum ProcessingMode {
    /// Produce samples realtime
    Easy,
    /// Fill up buffer up to `optimal_cap`
    Optimal,
    /// Fill up buffer up to `optimal_cap` using batch processing
    BatchOptimized,
    /// Fill up buffer up to `high_cap` using batch processing
    Bulk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProductionDelta {
    fill_size: usize,
    frames_per_buffer_size: usize,
    duration: Duration,
    mode: ProcessingMode,
    accumulated_latency: Duration,
    underruns_count: u32,
}

#[derive(Debug, Clone, Copy)]
struct Ema {
    value: f64,
    alpha: f64,
}

impl Ema {
    fn new(initial: f64, alpha: f64) -> Self {
        Self {
            value: initial,
            alpha,
        }
    }
    fn update(&mut self, sample: f64) {
        self.value = self.alpha * sample + (1.0 - self.alpha) * self.value;
    }
    fn get(&self) -> f64 {
        self.value
    }
}

// Helpers to convert between duration and frames for clarity
fn duration_to_frames(duration: Duration, sample_rate: u32) -> usize {
    (duration.as_secs_f64() * sample_rate as f64).ceil() as usize
}

fn frames_to_duration(frames: u32, sample_rate: u32) -> Duration {
    Duration::from_secs_f64(frames as f64 / sample_rate as f64)
}

const UNDERRUN_WINDOW_SECS: f64 = 3.0;
const UNDERRUN_RESET_THRESHOLD: u32 = 5;

pub fn playback_callback(
    net: NetBackend,
    input_buffer: Option<Arc<ThingBuf<S>>>,
    is_batch_processing: Arc<RwLock<bool>>,
    sample_rate: u32,
) -> Box<super::GenType> {
    let mut backend = BigBlockAdapter::new(Box::new(net));
    // Causal latency in (fractional) samples. After a reset, we can discard this many samples from the output to avoid incurring a pre-delay. The latency may depend on the sample rate.
    let net_latency = backend.latency();

    let buffer_capacity = ((sample_rate as f64 * BASE_MAX_BUFFER_MILLIS) / 1_000.0).ceil() as usize;
    let optimal_duration = Duration::from_secs_f64((BASE_OPTIMAL_BUFFER_MILLIS / 1_000.0) / 2.0);
    let sub_optimal_duration = Duration::from_secs_f64(BASE_OPTIMAL_BUFFER_MILLIS / 1_000.0);
    let critical_duration = Duration::from_secs_f64((BASE_OPTIMAL_BUFFER_MILLIS / 1_000.0) * 2.0);

    let sample_duration = Duration::from_secs_f64(1.0 / sample_rate as f64);

    let initial_optimal_cap =
        (sample_rate as f64 * BASE_OPTIMAL_BUFFER_MILLIS / 1_000.0).ceil() as usize;
    let initial_max_cap = (sample_rate as f64 * BASE_MAX_BUFFER_MILLIS / 1_000.0).ceil() as usize;
    let initial_low_water = (initial_max_cap as f32 * LOW_WATER_RATIO).ceil() as usize;
    let initial_high_water = (initial_max_cap as f32 * HIGH_WATER_RATIO).ceil() as usize;

    log::info!("Starting playback with: buffer_capacity=[{buffer_capacity}], optimal_duration=[{optimal_duration:?}], sub_optimal_duration=[{sub_optimal_duration:?}], critical_duration=[{critical_duration:?}], sample_duration=[{sample_duration:?}], optimal_cap=[{initial_optimal_cap}], low_water=[{initial_low_water}], high_water=[{initial_high_water}], net_latency=[{net_latency:?}]");

    let mut output_buffer = VecDeque::<(f32, f32)>::with_capacity(buffer_capacity);
    let mut frames_per_output_buffer = 64_usize;
    let mut production_delta = Option::<ProductionDelta>::None;
    let mut lr_frame_scratch = [0.0; 2];
    let mut scratch_left = vec![0_f32; buffer_capacity];
    let mut scratch_right = vec![0_f32; buffer_capacity];
    let mut scratch_input = vec![0_f32; buffer_capacity];

    // Adaptive telemetry
    // - ema_prod_ns: moving average of production duration per callback
    // - ema_avail_ns: moving average of available callback-to-playback slack
    // - ema_jitter_ns: moving average of absolute jitter of available slack
    // Alpha ~ 0.1: responds within ~10 cycles without being twitchy.
    let mut ema_prod_ns = Ema::new(0.0, 0.1);
    let mut ema_avail_ns = Ema::new(0.0, 0.1);
    let mut ema_jitter_ns = Ema::new(0.0, 0.1);
    let mut last_avail_ns: Option<f64> = None;

    // warm up backend and discard initial samples according to latency
    {
        let fill_size = net_latency
            .map(|lat| lat.ceil() as usize + initial_optimal_cap)
            .unwrap_or(initial_optimal_cap);

        batch_fill_size(
            &mut scratch_left,
            &mut scratch_right,
            &mut scratch_input,
            fill_size,
            frames_per_output_buffer,
            &mut backend,
            &input_buffer,
            &mut output_buffer,
        );

        output_buffer.clear();
    }

    // Underrun time window tracking (reset if too many underruns within the window)
    let mut underrun_window_start = std::time::Instant::now();

    Box::new(
        move |timestamp: OutputStreamTimestamp, frames: &mut [&mut [f32]]| {
            let start_time = std::time::Instant::now();
            let available_time = timestamp.playback.duration_since(&timestamp.callback);
            let (l_frames, r_frames) = frames.split_at_mut(1);
            let num_frames = l_frames[0].len();
            if num_frames != frames_per_output_buffer {
                log::info!(
                    "Output buffer size changed from [{frames_per_output_buffer}] to [{num_frames}] frames."
                );
                frames_per_output_buffer = num_frames;
            }

            let mut local_underruns = 0u32;
            for (i, (l, r)) in l_frames[0]
                .iter_mut()
                .zip(r_frames[0].iter_mut())
                .enumerate()
            {
                // local_underruns declared above
                let frame = match output_buffer.pop_front() {
                    Some(f) => f,
                    None => {
                        // Underrun: output silence, account for drift
                        *l = 0.0;
                        *r = 0.0;
                        local_underruns = local_underruns.saturating_add(1);
                        (0.0, 0.0)
                    }
                };

                if frame.0.is_nan() || frame.1.is_nan() {
                    log::warn!("NaN sample detected in playback buffer at frame {i}");
                } else if frame.0.is_infinite() || frame.1.is_infinite() {
                    log::warn!("Infinite sample detected in playback buffer at frame {i}");
                } else {
                    *l = frame.0;
                    *r = frame.1;
                }
            }

            // Update slack telemetry
            let avail_ns_opt = available_time.map(|d| d.as_nanos() as f64);
            if let Some(av_ns) = avail_ns_opt {
                ema_avail_ns.update(av_ns);
                if let Some(prev) = last_avail_ns {
                    ema_jitter_ns.update((av_ns - prev).abs());
                }
                last_avail_ns = Some(av_ns);
            }

            // Adapt buffer caps based on telemetry while preserving real-time feel.
            // Heuristics:
            // - If production often approaches/exceeds available slack, increase optimal/max caps up to 2x.
            // - If slack is plentiful and stable (low jitter), allow shrinking down to 0.5x.
            // - Keep low/high water in sync with max_cap.
            let prod_ns = production_delta
                .map(|d| d.duration.as_nanos() as f64)
                .unwrap_or_else(|| ema_prod_ns.get());
            if prod_ns > 0.0 {
                ema_prod_ns.update(prod_ns);
            }

            let avail_ns = ema_avail_ns.get();
            let ratio = if avail_ns > 0.0 {
                ema_prod_ns.get() / avail_ns
            } else {
                // If we don't know avail yet, be conservative.
                1.0
            };

            // Jitter normalization: compare jitter to average available
            let jitter_ratio = if avail_ns > 0.0 {
                (ema_jitter_ns.get() / avail_ns).min(2.0)
            } else {
                1.0
            };

            // Base scale from production pressure
            // - ratio ~0.5 => scale ~0.75
            // - ratio ~1.0 => scale ~1.25
            // - ratio ~2.0 => scale ~1.75
            let pressure_scale = 0.75 + 0.5 * ratio.clamp(0.0, 2.0);

            // Additional scale when jitter is high
            // - jitter_ratio ~0 => +0.0
            // - jitter_ratio ~1 => +0.15
            // - jitter_ratio ~2 => +0.3
            let jitter_scale = 1.0 + 0.15 * jitter_ratio;

            let mut adaptive_scale = pressure_scale * jitter_scale;

            // Clamp for real-time feel
            adaptive_scale = adaptive_scale.clamp(0.5, 2.0);

            let base_optimal_samples =
                (sample_rate as f64 * BASE_OPTIMAL_BUFFER_MILLIS / 1_000.0).ceil();
            let base_max_samples = (sample_rate as f64 * BASE_MAX_BUFFER_MILLIS / 1_000.0).ceil();

            let mut adaptive_optimal_cap = (base_optimal_samples * adaptive_scale).ceil() as usize;
            let adaptive_max_cap = (base_max_samples * adaptive_scale).ceil() as usize;

            if adaptive_optimal_cap > adaptive_max_cap {
                adaptive_optimal_cap = adaptive_max_cap;
            }

            // Recompute watermarks from adapted max cap
            let adaptive_low_water = (adaptive_max_cap as f32 * LOW_WATER_RATIO).ceil() as usize;
            let adaptive_high_water = (adaptive_max_cap as f32 * HIGH_WATER_RATIO).ceil() as usize;

            // Catch-up rule: if accumulated latency exceeds threshold, produce enough to cover
            // accumulated latency plus the optimal buffer, then reset production delta and proceed in Optimal mode.
            let prev_accumulated_latency = production_delta
                .as_ref()
                .map(|d| d.accumulated_latency)
                .unwrap_or(Duration::ZERO);

            let mut did_catch_up = false;

            // Detect current underruns for this callback before deciding production.
            // We don't consume frames yet; we only measure deficit to update accumulated latency early.
            let mut local_underruns_probe = 0u32;
            if output_buffer.len() < frames_per_output_buffer {
                local_underruns_probe = (frames_per_output_buffer - output_buffer.len()) as u32;
            }
            // Update accumulated latency immediately with probe result so catch-up decision sees latest drift.
            let catch_up_accumulated_latency = prev_accumulated_latency
                .saturating_add(frames_to_duration(local_underruns_probe, sample_rate));

            let (mode, fill_size) = if catch_up_accumulated_latency > sub_optimal_duration {
                // Convert accumulated latency to frames, add optimal cap
                let latency_frames = duration_to_frames(prev_accumulated_latency, sample_rate);
                let mut catch_up_fill = latency_frames + adaptive_optimal_cap;

                // Clamp so we don't exceed high water
                let remaining_capacity = adaptive_high_water.saturating_sub(output_buffer.len());
                catch_up_fill = catch_up_fill.min(remaining_capacity);

                did_catch_up = true;
                log::debug!(
                    "Playback catch-up: latency_frames={}, optimal_cap={}, fill={}, remaining_capacity={}",
                    latency_frames,
                    adaptive_optimal_cap,
                    catch_up_fill,
                    remaining_capacity
                );
                (ProcessingMode::Optimal, catch_up_fill)
            } else {
                determine_mode_and_fill_in_size(
                    available_time,
                    &production_delta,
                    frames_per_output_buffer,
                    output_buffer.len(),
                    adaptive_low_water,
                    adaptive_optimal_cap,
                    adaptive_high_water,
                )
            };

            match mode {
                ProcessingMode::Easy | ProcessingMode::Optimal => {
                    for _ in 0..fill_size {
                        #[allow(clippy::unnecessary_cast)]
                        let input = input_buffer
                            .as_ref()
                            .and_then(|ib| ib.pop())
                            .unwrap_or_default() as f32;
                        backend.tick(&[input], &mut lr_frame_scratch);
                        output_buffer.push_back((lr_frame_scratch[0], lr_frame_scratch[1]));
                    }
                }
                ProcessingMode::BatchOptimized | ProcessingMode::Bulk => {
                    batch_fill_size(
                        &mut scratch_left,
                        &mut scratch_right,
                        &mut scratch_input,
                        fill_size,
                        frames_per_output_buffer,
                        &mut backend,
                        &input_buffer,
                        &mut output_buffer,
                    );
                }
            }

            *is_batch_processing.write() = is_batch_for_mode(mode);

            // Update accumulated latency and underruns_count; reset both after catch-up
            let prev_underruns = production_delta
                .as_ref()
                .map(|d| d.underruns_count)
                .unwrap_or(0);
            let mut updated_underruns = if did_catch_up {
                0
            } else {
                prev_underruns.saturating_add(local_underruns)
            };

            let updated_accumulated_latency = if did_catch_up {
                Duration::ZERO
            } else {
                let added =
                    Duration::from_secs_f64(local_underruns as f64 * sample_duration.as_secs_f64());
                prev_accumulated_latency.saturating_add(added)
            };

            // Time-windowed underrun threshold: reset backend and counters if exceeded
            let underrun_window_secs: f64 = UNDERRUN_WINDOW_SECS;
            let underrun_reset_threshold: u32 = UNDERRUN_RESET_THRESHOLD;
            if underrun_window_start.elapsed().as_secs_f64() >= underrun_window_secs {
                if updated_underruns >= underrun_reset_threshold {
                    log::warn!(
                        "Frequent underruns detected (count={} in ~{}s). Resetting backend.",
                        updated_underruns,
                        underrun_window_secs
                    );
                    backend.reset();
                    output_buffer.clear();
                    updated_underruns = 0;

                    // Warm-up immediately to reduce audible gaps:
                    // Produce up to adaptive_optimal_cap frames (bounded by remaining capacity)
                    let remaining_capacity =
                        adaptive_high_water.saturating_sub(output_buffer.len());
                    let warmup_fill = adaptive_optimal_cap.min(remaining_capacity);
                    if warmup_fill > 0 {
                        batch_fill_size(
                            &mut scratch_left,
                            &mut scratch_right,
                            &mut scratch_input,
                            warmup_fill,
                            frames_per_output_buffer,
                            &mut backend,
                            &input_buffer,
                            &mut output_buffer,
                        );
                        log::debug!(
                            "Playback warm-up after reset: warmup_fill={}, remaining_capacity={}",
                            warmup_fill,
                            remaining_capacity
                        );
                    }

                    // After a reset, also clear accumulated latency
                    production_delta = Some(ProductionDelta {
                        fill_size,
                        frames_per_buffer_size: frames_per_output_buffer,
                        duration: start_time.elapsed(),
                        mode: ProcessingMode::Optimal,
                        accumulated_latency: Duration::ZERO,
                        underruns_count: updated_underruns,
                    });
                    // Restart the window and continue without early return; frames already filled this callback
                    underrun_window_start = std::time::Instant::now();
                }
                // restart window even if not resetting
                underrun_window_start = std::time::Instant::now();
                // decay the count across windows to avoid permanent buildup
                updated_underruns = updated_underruns.saturating_div(2);
            }

            production_delta = Some(ProductionDelta {
                fill_size,
                frames_per_buffer_size: frames_per_output_buffer,
                duration: start_time.elapsed(),
                mode: if did_catch_up {
                    ProcessingMode::Optimal
                } else {
                    mode
                },
                accumulated_latency: updated_accumulated_latency,
                underruns_count: updated_underruns,
            });
        },
    ) as Box<super::GenType>
}

fn determine_mode_and_fill_in_size(
    available_time: Option<Duration>,
    production_delta: &Option<ProductionDelta>,
    frames_per_output_buffer: usize,
    current_buffer_len: usize,
    low_cap: usize,
    optimal_cap: usize,
    high_cap: usize,
) -> (ProcessingMode, usize) {
    // Use last known production duration. If unknown, default to a conservative stance.
    let last_duration = production_delta.map(|d| d.duration);
    let last_mode = production_delta.map(|d| d.mode);

    // If we don't know the available callback-to-playback slack, default to BatchOptimized.
    // This favors deeper buffering over real-time minimalism.
    let base_mode = match (available_time, last_duration) {
        (None, _) => ProcessingMode::BatchOptimized,
        (Some(avail), Some(last)) => {
            // Compare how expensive the last production was versus the time we have now.
            // Heuristic thresholds:
            // - last <= 0.5 * avail => Easy (plenty of slack, produce minimally)
            // - last <= 1.0 * avail => Optimal (comfortable)
            // - last <= 2.0 * avail => BatchOptimized (borderline, deepen buffer)
            // - last > 2.0 * avail  => Bulk (we were too slow; catch up aggressively)
            let last_ns = last.as_nanos();
            let avail_ns = avail.as_nanos();
            if last_ns <= avail_ns / 2 {
                ProcessingMode::Easy
            } else if last_ns <= avail_ns {
                ProcessingMode::Optimal
            } else if last_ns <= avail_ns * 2 {
                ProcessingMode::BatchOptimized
            } else {
                ProcessingMode::Bulk
            }
        }
        (Some(_), None) => {
            // No prior measurement: pick BatchOptimized to stabilize quickly.
            ProcessingMode::BatchOptimized
        }
    };

    // Adjust mode based on buffer health and prior stress signal.
    let mut mode = base_mode;

    // If buffer is below low water, strengthen mode to increase fill (avoid underruns).
    if current_buffer_len < low_cap {
        mode = match mode {
            ProcessingMode::Easy | ProcessingMode::Optimal => ProcessingMode::BatchOptimized,
            ProcessingMode::BatchOptimized | ProcessingMode::Bulk => ProcessingMode::Bulk,
        };
    }

    // If previous mode was already Bulk and we’re not yet at optimal, stay aggressive.
    if matches!(last_mode, Some(ProcessingMode::Bulk)) && current_buffer_len < optimal_cap {
        mode = ProcessingMode::Bulk;
    }

    // Compute target buffer level based on mode.
    // We aim toward:
    // - Easy: maintain around frames_per_output_buffer (minimal latency)
    // - Optimal: around optimal_cap
    // - BatchOptimized: above optimal_cap but below high_cap
    // - Bulk: push toward high_cap
    let target_cap = match mode {
        ProcessingMode::Easy => frames_per_output_buffer.max(low_cap),
        ProcessingMode::Optimal => optimal_cap,
        ProcessingMode::BatchOptimized => ((optimal_cap + high_cap) / 2).max(optimal_cap),
        ProcessingMode::Bulk => high_cap,
    };

    // Determine how much to fill to move current_buffer_len toward target_cap.
    let deficit = target_cap.saturating_sub(current_buffer_len);

    // Always fill in multiples of frames_per_output_buffer to match generator granularity.
    // Also enforce a minimum of one buffer when we decide to fill.
    let mut fill_buffers = match mode {
        ProcessingMode::Easy => 1,
        ProcessingMode::Optimal => (deficit / frames_per_output_buffer).max(1),
        ProcessingMode::BatchOptimized => (deficit / frames_per_output_buffer).max(2),
        ProcessingMode::Bulk => (deficit / frames_per_output_buffer).max(4),
    };

    // If available_time is present and last production exceeded it, be more aggressive by one buffer.
    if available_time
        .and_then(|avail| last_duration.filter(|&last| last > avail))
        .is_some()
    {
        fill_buffers += 1;
    }

    // Prevent overfilling beyond high_cap.
    if current_buffer_len + fill_buffers * frames_per_output_buffer > high_cap {
        let max_buffers = (high_cap.saturating_sub(current_buffer_len)) / frames_per_output_buffer;
        fill_buffers = fill_buffers.min(max_buffers.max(1));
    }

    let fill_size = fill_buffers * frames_per_output_buffer;

    (mode, fill_size)
}

fn is_batch_for_mode(mode: ProcessingMode) -> bool {
    matches!(mode, ProcessingMode::BatchOptimized | ProcessingMode::Bulk)
}

#[allow(clippy::too_many_arguments)]
fn batch_fill_size(
    scratch_left: &mut [f32],
    scratch_right: &mut [f32],
    scratch_input: &mut [f32],
    fill_size: usize,
    chunk_size: usize,
    backend: &mut BigBlockAdapter,
    input_buffer: &Option<Arc<ThingBuf<S>>>,
    output_buffer: &mut VecDeque<(f32, f32)>,
) {
    if fill_size == 0 {
        return;
    }

    let rem = fill_size % chunk_size;
    let complete_chunks = fill_size / chunk_size;

    for chunk in 0..complete_chunks {
        let offset = chunk * chunk_size;
        batch_fill_size_inner(
            &mut scratch_left[offset..],
            &mut scratch_right[offset..],
            &mut scratch_input[offset..],
            chunk_size,
            backend,
            input_buffer,
            output_buffer,
        );
    }

    if rem > 0 {
        let offset = complete_chunks * chunk_size;
        batch_fill_size_inner(
            &mut scratch_left[offset..],
            &mut scratch_right[offset..],
            &mut scratch_input[offset..],
            rem,
            backend,
            input_buffer,
            output_buffer,
        );
    }

    #[allow(clippy::unnecessary_cast)]
    fn batch_fill_size_inner(
        scratch_left: &mut [f32],
        scratch_right: &mut [f32],
        scratch_input: &mut [f32],
        fill_size: usize,
        backend: &mut BigBlockAdapter,
        input_buffer: &Option<Arc<ThingBuf<S>>>,
        output_buffer: &mut VecDeque<(f32, f32)>,
    ) {
        let mut frames_per_channel = [
            &mut scratch_left[..fill_size],
            &mut scratch_right[..fill_size],
        ];

        if let Some(ib) = &input_buffer {
            for input_val in scratch_input.iter_mut().take(fill_size) {
                *input_val = ib.pop().unwrap_or_default() as f32;
            }
        }

        backend.process_big(
            fill_size,
            &[&scratch_input[..fill_size]],
            &mut frames_per_channel,
        );

        for (&l, &r) in scratch_left[..fill_size]
            .iter()
            .zip(scratch_right[..fill_size].iter())
        {
            output_buffer.push_back((l, r));
        }
    }
}
