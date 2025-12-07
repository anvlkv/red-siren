use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{mpsc, Arc};
use std::thread::{sleep, spawn};
use std::time::Duration;

use cpal::StreamInstant;
use fundsp::hacker::{AudioUnit, BigBlockAdapter, NetBackend};
use fundsp::thingbuf::ThingBuf;
use parking_lot::RwLock;

use super::Control;
use crate::util::S;

const OPTIMAL_BUFFER_MILLIS: f64 = 80.0;
const MAX_BUFFER_MILLIS: f64 = 320.0;
const HIGH_WATER_RATIO: f32 = 0.9;
const LOW_WATER_RATIO: f32 = 0.2;

// Hysteresis tuning:
// - Require this many consecutive "healthy" intervals to relax mode (downshift)
// - Immediately upshift on underrun; otherwise upshift on latency breach for a few consecutive intervals
const HEALTHY_STREAK_DOWN_SHIFT: usize = 7;
const UNHEALTHY_STREAK_UP_SHIFT: usize = 2;

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

pub fn spawn_playback_stream(
    net: NetBackend,
    input_buffer: Option<Arc<ThingBuf<S>>>,
    is_batch_processing: Arc<RwLock<bool>>,
    sample_rate: u32,
) -> (
    Box<super::GenType>,
    Sender<Control>,
    std::thread::JoinHandle<()>,
) {
    let mut backend = BigBlockAdapter::new(Box::new(net));
    // Causal latency in (fractional) samples. After a reset, we can discard this many samples from the output to avoid incurring a pre-delay. The latency may depend on the sample rate.
    let net_latency = backend.latency();

    let buffer_capacity = ((sample_rate as f64 * MAX_BUFFER_MILLIS) / 1_000.0).ceil() as usize;
    let optimal_duration = Duration::from_secs_f64((OPTIMAL_BUFFER_MILLIS / 1_000.0) / 2.0);
    let sub_optimal_duration = Duration::from_secs_f64(OPTIMAL_BUFFER_MILLIS / 1_000.0);
    let critical_duration = Duration::from_secs_f64((OPTIMAL_BUFFER_MILLIS / 1_000.0) * 2.0);

    let sample_duration = Duration::from_secs_f64(1.0 / sample_rate as f64);

    let low_water = (buffer_capacity as f32 * LOW_WATER_RATIO).ceil() as usize;
    let optimal_cap = (sample_rate as f64 * OPTIMAL_BUFFER_MILLIS / 1_000.0).ceil() as usize;
    let high_water = (buffer_capacity as f32 * HIGH_WATER_RATIO).ceil() as usize;

    log::info!("Starting playback with: buffer_capacity=[{buffer_capacity}], optimal_duration=[{optimal_duration:?}], sub_optimal_duration=[{sub_optimal_duration:?}], critical_duration=[{critical_duration:?}], sample_duration=[{sample_duration:?}], optimal_cap=[{optimal_cap}], low_water=[{low_water}], high_water=[{high_water}], net_latency=[{net_latency:?}]");

    let time_stamps_and_underrun = Arc::new(ThingBuf::<Option<(StreamInstant, i32)>>::new(
        buffer_capacity,
    ));
    let buffer = Arc::new(ThingBuf::<(f32, f32)>::new(buffer_capacity));
    let frames_per_output_buffer = Arc::new(AtomicUsize::new(64));

    let buffer_producer = buffer.clone();
    let time_stamps_producer = time_stamps_and_underrun.clone();
    let frames_per_output_buffer_producer = frames_per_output_buffer.clone();

    let (tx, rx): (Sender<Control>, Receiver<Control>) = mpsc::channel();

    let producer_handle = spawn(move || {
        let mut last_ts: Option<(StreamInstant, isize)> = None;
        let mut lr_frame_scratch = [0.0; 2];
        let mut scratch_left = vec![0_f32; buffer_capacity];
        let mut scratch_right = vec![0_f32; buffer_capacity];
        let mut scratch_input = vec![0_f32; buffer_capacity];

        // warm up backend and discard initial samples according to latency
        if let Some(lat) = net_latency {
            let fill_size = lat.ceil() as usize;

            batch_fill_size(
                &mut scratch_left,
                &mut scratch_right,
                &mut scratch_input,
                fill_size,
                frames_per_output_buffer_producer.load(Ordering::Relaxed),
                &mut backend,
                &input_buffer,
                &buffer_producer,
            );

            while buffer_producer.pop_ref().is_some() {
                // pop warm up samples
            }
        }

        // Hysteresis controller state
        let mut current_mode = ProcessingMode::BatchOptimized; // conservative default until timestamps
        let mut healthy_streak = 0usize;
        let mut unhealthy_streak = 0usize;

        let mut next_tick = {
            let buffer_producer = buffer_producer.clone();
            let frames_per_output_buffer_producer = frames_per_output_buffer_producer.clone();
            move || -> Option<(ProcessingMode, i32)> {
                // handle playback control commands
                match rx.try_recv() {
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => {
                        log::error!("Playback control channel disconnected.");
                        return None;
                    }
                    Ok(Control::Shutdown(resp_x)) => {
                        _ = resp_x.send(Ok(()));
                        return None;
                    }
                    Ok(Control::Pause(resp_x)) => {
                        _ = resp_x.send(Ok(()));
                        if let Some(Control::Resume(resume_x)) =
                            rx.iter().find(|c| matches!(c, Control::Resume(_)))
                        {
                            _ = resume_x.send(Ok(()));
                        } else {
                            log::warn!("Paused playback was never resumed");
                            return None;
                        }
                    }
                    Ok(Control::Resume(resp_x)) => {
                        _ = resp_x.send(Ok(()));
                        log::warn!("Received unexpected Resume command while not paused");
                    }
                }

                // determine latency for next tick and underrun delta
                let prev_ts = last_ts.take();
                while let Some(&Some((ts, underrun))) = time_stamps_producer.pop_ref().as_deref() {
                    let (last_ts_ref, last_underrun_count) = last_ts.get_or_insert((ts, 0));
                    *last_ts_ref = ts;
                    *last_underrun_count += underrun as isize;
                }
                let delta_ts = prev_ts
                    .and_then(|(prev, _)| last_ts.and_then(|(last, _)| last.duration_since(&prev)));

                let delta_underrun = prev_ts.and_then(|(_, prev_underrun)| {
                    last_ts.map(|(_, last_underrun)| last_underrun - prev_underrun)
                });

                // Update hysteresis counters and decide tentative mode signal
                let mode_signal = compute_processing_signal(
                    delta_ts,
                    optimal_duration,
                    sub_optimal_duration,
                    critical_duration,
                    delta_underrun.map(|d| d as i32),
                );

                // Hysteresis: adjust healthy/unhealthy streaks.
                // Any positive underrun is "unhealthy" and triggers immediate upshift.
                let had_underrun = delta_underrun.map(|d| d > 0).unwrap_or(false);

                if had_underrun {
                    unhealthy_streak = UNHEALTHY_STREAK_UP_SHIFT; // force immediate upshift
                    healthy_streak = 0;
                } else {
                    // Healthy if timestamps within current mode's bounds
                    if is_healthy_for_mode(
                        delta_ts,
                        current_mode,
                        optimal_duration,
                        sub_optimal_duration,
                        critical_duration,
                    ) {
                        healthy_streak = healthy_streak.saturating_add(1);
                        unhealthy_streak = 0;
                    } else {
                        unhealthy_streak = unhealthy_streak.saturating_add(1);
                        healthy_streak = 0;
                    }
                }

                // Apply hysteresis rules:
                // - Upshift if unhealthy streak exceeds threshold OR any underrun occurred
                // - Downshift only after enough consecutive healthy intervals
                current_mode = match (current_mode, mode_signal) {
                    // Upshift path: go to the stronger of current vs signal when unhealthy
                    (mode, signal)
                        if had_underrun || unhealthy_streak >= UNHEALTHY_STREAK_UP_SHIFT =>
                    {
                        unhealthy_streak = 0; // consume
                        stronger_mode(mode, signal)
                    }
                    // Downshift path: only relax after sustained health
                    (mode, signal) if healthy_streak >= HEALTHY_STREAK_DOWN_SHIFT => {
                        healthy_streak = 0; // consume
                        weaker_mode(mode, signal)
                    }
                    // Otherwise, keep current mode (prevent hysterical switching)
                    (mode, _) => mode,
                };

                if let Some(mut is_batch_processing) = is_batch_processing.try_write() {
                    *is_batch_processing = is_batch_for_mode(current_mode);
                }

                let frames_per_buffer = frames_per_output_buffer_producer.load(Ordering::Relaxed);

                let optimal_cap = optimal_cap - (optimal_cap % frames_per_buffer);
                let high_cap = high_water - (high_water % frames_per_buffer);

                // determine fill size for next tick
                let buffer_len = buffer_producer.len();
                match (buffer_len, current_mode) {
                    // urgent refill
                    (buffer_len, _) if buffer_len <= low_water => Some((
                        ProcessingMode::Bulk,
                        (optimal_cap.saturating_sub(buffer_len) as i32)
                            .max(frames_per_buffer as i32),
                    )),
                    // single-buffer processing
                    (buffer_len, ProcessingMode::Easy) if buffer_len < optimal_cap => {
                        Some((current_mode, frames_per_buffer as i32))
                    }
                    // fill to optimal using single-buffer processing or batch processing
                    (buffer_len, ProcessingMode::Optimal)
                    | (buffer_len, ProcessingMode::BatchOptimized)
                        if buffer_len < optimal_cap =>
                    {
                        Some((
                            current_mode,
                            (optimal_cap.saturating_sub(buffer_len) as i32)
                                .max(frames_per_buffer as i32),
                        ))
                    }
                    // bulk processing
                    (buffer_len, ProcessingMode::Bulk) if buffer_len < high_cap => Some((
                        current_mode,
                        (high_cap.saturating_sub(buffer_len) as i32).max(frames_per_buffer as i32),
                    )),
                    // overrun in any other case
                    _ => Some((
                        current_mode,
                        match current_mode {
                            ProcessingMode::Easy => {
                                -(buffer_len.saturating_sub(frames_per_buffer) as i32)
                            }
                            ProcessingMode::Optimal | ProcessingMode::BatchOptimized => {
                                -(buffer_len.saturating_sub(optimal_cap) as i32)
                            }
                            ProcessingMode::Bulk => -(buffer_len.saturating_sub(high_cap) as i32),
                        },
                    )),
                }
            }
        };

        'main: while let Some((mode, fill_size)) = next_tick() {
            if fill_size <= 0 {
                log::info!("Playback thread overrun in mode {:?}", mode);
                sleep(sample_duration * (-fill_size / 3) as u32);
                continue 'main;
            }
            match mode {
                ProcessingMode::Easy | ProcessingMode::Optimal => {
                    'inner: for _ in 0..fill_size {
                        #[allow(clippy::unnecessary_cast)]
                        let input = input_buffer
                            .as_ref()
                            .and_then(|ib| ib.pop())
                            .unwrap_or_default() as f32;
                        backend.tick(&[input], &mut lr_frame_scratch);
                        if let Ok(mut place) = buffer_producer.push_ref() {
                            *place = (lr_frame_scratch[0], lr_frame_scratch[1]);
                        } else {
                            break 'inner;
                        }
                    }
                }
                ProcessingMode::BatchOptimized | ProcessingMode::Bulk => {
                    batch_fill_size(
                        &mut scratch_left,
                        &mut scratch_right,
                        &mut scratch_input,
                        fill_size as usize,
                        frames_per_output_buffer_producer.load(Ordering::Relaxed),
                        &mut backend,
                        &input_buffer,
                        &buffer_producer,
                    );
                }
            }
        }

        log::info!("Playback thread exiting.");
    });

    let producer_fn = Box::new(move |instant: StreamInstant, frames: &mut [&mut [f32]]| {
        let (l_frames, r_frames) = frames.split_at_mut(1);
        let num_frames = l_frames[0].len();
        frames_per_output_buffer.store(num_frames, Ordering::Relaxed);

        // Count exactly how many frames we were able to fill this callback.
        let mut frames_filled = 0usize;

        let mut last_frame = None;

        for (i, (l, r)) in l_frames[0]
            .iter_mut()
            .zip(r_frames[0].iter_mut())
            .enumerate()
        {
            if let Some(frame) = buffer.pop_ref().as_deref().copied() {
                if frame.0.is_nan() || frame.1.is_nan() {
                    log::warn!("NaN sample detected in playback buffer at frame {i}");
                } else if frame.0.is_infinite() || frame.1.is_infinite() {
                    log::warn!("Infinite sample detected in playback buffer at frame {i}");
                } else {
                    *l = frame.0;
                    *r = frame.1;
                    last_frame = Some(frame);
                }
                frames_filled = i + 1;
            } else if let Some((last_l, last_r)) = last_frame.map(|f| {
                let spread = (num_frames - (num_frames - i + 1)) as f32;
                (f.0 / spread, f.1 / spread)
            }) {
                *l = last_l;
                *r = last_r;
            } else {
                break;
            }
        }

        // Underrun is exactly the number of frames not produced this callback.
        let underrun: i32 = (num_frames - frames_filled) as i32;

        if underrun > 0 {
            log::warn!(
                "Playback underrun: requested [{num_frames}] frames, filled [{frames_filled}] frames. Underrun=[{underrun}]"
            );
        }

        if let Ok(mut place) = time_stamps_and_underrun.push_ref().or_else(|_| {
            _ = time_stamps_and_underrun.pop_ref();
            time_stamps_and_underrun.push_ref()
        }) {
            *place = Some((instant, underrun));
        }
    }) as Box<super::GenType>;

    (producer_fn, tx, producer_handle)
}

/// Convert instantaneous signal into a target processing mode (without hysteresis).
/// - If `delta_underrun > 0` escalate immediately to `Bulk`
/// - Otherwise, base on timestamp delta thresholds.
fn compute_processing_signal(
    delta: Option<Duration>,
    optimal_duration: Duration,
    sub_optimal_duration: Duration,
    critical_duration: Duration,
    delta_underrun: Option<i32>,
) -> ProcessingMode {
    // If there were underruns in the last interval, escalate aggressively.
    if delta_underrun.is_some_and(|d| d > 0) {
        return ProcessingMode::Bulk;
    }

    match delta {
        Some(d) if d <= optimal_duration => ProcessingMode::Easy,
        Some(d) if d <= sub_optimal_duration => ProcessingMode::Optimal,
        Some(d) if d <= critical_duration => ProcessingMode::BatchOptimized,
        Some(_) => ProcessingMode::Bulk,
        None => {
            // No timestamps observed yet; conservatively batch to quickly establish buffer health.
            ProcessingMode::BatchOptimized
        }
    }
}

/// Determine whether current timestamp delta is "healthy" for the given mode.
/// Health means: the observed callback spacing is within or better than mode's expected range.
fn is_healthy_for_mode(
    delta: Option<Duration>,
    mode: ProcessingMode,
    optimal_duration: Duration,
    sub_optimal_duration: Duration,
    critical_duration: Duration,
) -> bool {
    match (delta, mode) {
        (None, _) => false,
        (Some(d), ProcessingMode::Easy) => d <= optimal_duration,
        (Some(d), ProcessingMode::Optimal) => d <= sub_optimal_duration,
        (Some(d), ProcessingMode::BatchOptimized) => d <= critical_duration,
        (Some(_), ProcessingMode::Bulk) => true, // Bulk is the most conservative; any delta is acceptable
    }
}

/// Pick the stronger of two modes (stronger means more aggressive in buffering/throughput).
fn stronger_mode(a: ProcessingMode, b: ProcessingMode) -> ProcessingMode {
    use ProcessingMode::*;
    match (a, b) {
        (Bulk, _) | (_, Bulk) => Bulk,
        (BatchOptimized, _) | (_, BatchOptimized) => BatchOptimized,
        (Optimal, _) | (_, Optimal) => Optimal,
        _ => Easy,
    }
}

/// Pick the weaker of two modes (weaker means more real-time, less buffering).
fn weaker_mode(a: ProcessingMode, b: ProcessingMode) -> ProcessingMode {
    use ProcessingMode::*;
    match (a, b) {
        (Easy, _) | (_, Easy) => Easy,
        (Optimal, _) | (_, Optimal) => Optimal,
        (BatchOptimized, _) | (_, BatchOptimized) => BatchOptimized,
        _ => Bulk,
    }
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
    buffer_producer: &Arc<ThingBuf<(f32, f32)>>,
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
            buffer_producer,
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
            buffer_producer,
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
        buffer_producer: &Arc<ThingBuf<(f32, f32)>>,
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
            if let Ok(mut place) = buffer_producer.push_ref() {
                *place = (l, r);
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(ms: u64) -> std::time::Duration {
        std::time::Duration::from_millis(ms)
    }

    #[test]
    fn signal_easy_when_delta_under_optimal_and_no_underrun() {
        let mode = compute_processing_signal(Some(d(30)), d(60), d(125), d(250), Some(0));
        assert_eq!(mode, ProcessingMode::Easy);
        assert!(!is_batch_for_mode(mode));
    }

    #[test]
    fn signal_bulk_on_underrun_even_if_delta_is_good() {
        let mode = compute_processing_signal(Some(d(30)), d(60), d(125), d(250), Some(1));
        assert_eq!(mode, ProcessingMode::Bulk);
        assert!(is_batch_for_mode(mode));
    }

    #[test]
    fn signal_optimal_when_delta_under_sub_optimal() {
        let mode = compute_processing_signal(Some(d(80)), d(60), d(125), d(250), Some(0));
        assert_eq!(mode, ProcessingMode::Optimal);
        assert!(!is_batch_for_mode(mode));
    }

    #[test]
    fn signal_batch_optimized_when_delta_under_critical() {
        let mode = compute_processing_signal(Some(d(180)), d(60), d(125), d(250), Some(0));
        assert_eq!(mode, ProcessingMode::BatchOptimized);
        assert!(is_batch_for_mode(mode));
    }

    #[test]
    fn signal_bulk_when_delta_over_critical() {
        let mode = compute_processing_signal(Some(d(400)), d(60), d(125), d(250), Some(0));
        assert_eq!(mode, ProcessingMode::Bulk);
        assert!(is_batch_for_mode(mode));
    }

    #[test]
    fn signal_none_defaults_to_batch_optimized() {
        let mode = compute_processing_signal(None, d(60), d(125), d(250), Some(0));
        assert_eq!(mode, ProcessingMode::BatchOptimized);
        assert!(is_batch_for_mode(mode));
    }
}
