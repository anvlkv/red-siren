use std::collections::VecDeque;
use std::iter;
use std::sync::Arc;
use std::time::Duration;

use common::instrument::PlaybackQuality;
use cpal::OutputStreamTimestamp;
use fundsp::prelude::{AudioUnit, BigBlockAdapter, NetBackend};
use fundsp::thingbuf::ThingBuf;

use crate::util::S;

const BASE_OPTIMAL_BUFFER_MILLIS: f64 = 60.0;
const MAX_SILENCE: f64 = 3.0;

/// Convert a duration to frame count at a given sample rate.
fn duration_to_frames(d: Duration, sample_rate: u32) -> usize {
    ((d.as_secs_f64() * sample_rate as f64).ceil()) as usize
}

#[allow(clippy::unnecessary_cast)]
pub fn playback_callback(
    net: NetBackend,
    input_buffer: Option<Arc<ThingBuf<f32>>>,
    quality: Arc<std::sync::atomic::AtomicI8>,
    no_reset_on_silence: Arc<parking_lot::RwLock<bool>>,
    sample_rate: u32,
) -> Box<super::GenType> {
    let mut backend = BigBlockAdapter::new(Box::new(net));
    let net_latency = backend.latency();

    // Single optimal target
    let optimal_cap = (sample_rate as f64 * BASE_OPTIMAL_BUFFER_MILLIS / 1_000.0).ceil() as usize;
    let sub_optimal_duration = Duration::from_secs_f64(BASE_OPTIMAL_BUFFER_MILLIS / 1_000.0);
    let sample_duration = Duration::from_secs_f64(1.0 / sample_rate as f64);

    let mut lr_frame_scratch = [0.0; 2];
    let mut i_batch_scratch = vec![0.0_f32; 512];
    let mut l_batch_scratch = vec![0.0_f32; 512];
    let mut r_batch_scratch = vec![0.0_f32; 512];
    let mut output_buffer = VecDeque::<(f32, f32)>::with_capacity(optimal_cap);
    let mut frames_per_output_buffer = 64_usize;

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

    Box::new(move |_: OutputStreamTimestamp, frames: &mut [&mut [f32]]| {
        let mut inner_quality_indicator = Option::<PlaybackQuality>::None;
        let (l_frames, r_frames) = frames.split_at_mut(1);
        let num_frames = l_frames[0].len();
        if num_frames != frames_per_output_buffer {
            frames_per_output_buffer = num_frames;
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

        if accumulated_silence >= Duration::from_secs_f64(MAX_SILENCE) {
            log::warn!("Resetting network. Accumulated silence: {accumulated_silence:?}");
            backend.reset();
            accumulated_silence = Duration::ZERO;
            _ = inner_quality_indicator.get_or_insert(PlaybackQuality::Resetting);
        }

        // Update latency from this callback's underruns
        if local_underruns > 0 {
            let added =
                Duration::from_secs_f64(local_underruns as f64 * sample_duration.as_secs_f64());
            accumulated_latency = accumulated_latency.saturating_add(added);
            _ = inner_quality_indicator.get_or_insert(PlaybackQuality::Underruns);
        }

        // Decide fill_size:
        // - If we need catch-up (accumulated latency exceeds sub-optimal duration),
        //   fill extra: latency_frames + optimal_cap (clamped to remaining capacity).
        // - Else, fill toward optimal level.
        let current_len = output_buffer.len();
        let remaining_capacity = optimal_cap.saturating_sub(current_len);

        let fill_size = if accumulated_latency > sub_optimal_duration {
            let latency_frames = duration_to_frames(accumulated_latency, sample_rate);
            let catch_up = latency_frames.saturating_add(optimal_cap);
            _ = inner_quality_indicator.get_or_insert(PlaybackQuality::OptimizedQuality);
            catch_up.min(remaining_capacity)
        } else {
            _ = inner_quality_indicator.get_or_insert(PlaybackQuality::HighQuality);
            optimal_cap
                .saturating_sub(current_len)
                .min(remaining_capacity)
        }
        .max(num_frames);

        if inner_quality_indicator.is_some_and(|q| {
            matches!(
                q,
                PlaybackQuality::OptimizedQuality | PlaybackQuality::Underruns
            )
        }) {
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
            output_buffer.extend((0..fill_size).map(|i| (l_batch_scratch[i], r_batch_scratch[i])));
        } else {
            for _ in 0..fill_size {
                let input = input_buffer
                    .as_ref()
                    .and_then(|ib| ib.pop())
                    .unwrap_or_default();
                backend.tick(&[input], &mut lr_frame_scratch);
                output_buffer.push_back((lr_frame_scratch[0], lr_frame_scratch[1]));
            }
        }

        // After catch-up production, reset accumulated latency
        if accumulated_latency > sub_optimal_duration {
            accumulated_latency = Duration::ZERO;
        }

        if let Some(q) = inner_quality_indicator {
            quality_indicator.store(q as u8, std::sync::atomic::Ordering::Relaxed);
        }
    }) as Box<super::GenType>
}
