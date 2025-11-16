//! CPAL output stream owner & helpers.
//!
//! Responsibilities:
//! - Owns the CPAL output stream on a dedicated thread.
//! - Generates interleaved audio samples via a caller-supplied closure.
//! - Provides pause / resume / shutdown control with ACK semantics.
//!
//! Design (MAYA DRY KISS):
//! - Keep lifetime + thread affinity constraints isolated here.
//! - Avoid moving `Stream` across threads (create + control in owner thread).
//! - Minimal surface: only what the current runtime requires.
//!
//! Extending:
//! - Add new control messages only if required by multiple components.
//! - Keep per-format branching centralized in `run_output`.
//!
//! Safety / Correctness Notes:
//! - CPAL callbacks are real-time; we avoid allocations inside `write_data`
//!   except those performed by the sample generator closure itself.
//! - Logging at `trace` level only; higher levels would risk timing.
use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use common::error::InstrumentError;
use cpal::{
    traits::{DeviceTrait, StreamTrait},
    SampleFormat, Stream, StreamConfig, StreamInstant, SupportedStreamConfig,
};

use super::{Control, ControlInvocationResult, STREAM_TIMEOUT_S};

/// Stereo (L,R) sample generator invoked per audio frame or with batch.
pub type GenType = dyn FnMut(Option<(&mut [&mut [f32]], usize)>) -> Option<(f32, f32)> + Send;

/// Spawn an owner thread that creates and owns a CPAL output stream.
///
/// Parameters:
/// - `device`, `default_cfg`, `stream_cfg`: moved into the owner thread.
/// - `channels`: number of output channels to fill (>= 2, we duplicate L/R across if needed).
/// - `make_next`: closure executed on owner thread returning a boxed generator.
///
/// Returns:
/// - Control sender for issuing Pause/Resume/Shutdown.
/// - JoinHandle that can be joined after Shutdown.
///
/// Errors:
/// - Stream creation / play failures surfaced as `InstrumentError`.
pub fn spawn_owned_output_stream<FMake>(
    device: cpal::Device,
    default_cfg: SupportedStreamConfig,
    stream_cfg: StreamConfig,
    channels: usize,
    make_next: FMake,
) -> Result<(Sender<Control>, thread::JoinHandle<()>), InstrumentError>
where
    FMake: Send + 'static + FnOnce() -> Box<GenType>,
{
    let (tx, rx): (Sender<Control>, Receiver<Control>) = mpsc::channel();
    let (init_tx, init_rx) = mpsc::channel::<Result<(), InstrumentError>>();

    let handle = thread::spawn(move || {
        // Build generator inside owner thread to keep its captures thread-affine if needed.
        let next = make_next();

        // Build CPAL stream on this thread.
        let stream = match run_output(&device, &stream_cfg, &default_cfg, channels, next) {
            Ok(s) => s,
            Err(e) => {
                let _ = init_tx.send(Err(InstrumentError::StartFailed {
                    detail: Some(e.to_string()),
                }));
                return;
            }
        };

        if let Err(e) = stream.play() {
            let _ = init_tx.send(Err(InstrumentError::StartFailed {
                detail: Some(e.to_string()),
            }));
            return;
        }

        // Signal success.
        let _ = init_tx.send(Ok(()));

        // Control loop (blocks on receiver, keeping stream alive).
        while let Ok(msg) = rx.recv() {
            match msg {
                Control::Pause(ret) => {
                    _ = match stream.pause() {
                        Ok(_) => ret.send(ControlInvocationResult::Ok(())),
                        Err(e) => ret.send(ControlInvocationResult::Err(e.to_string())),
                    }
                    .inspect_err(|e| log::error!("error sending pause ack: {e}"));
                }
                Control::Resume(ret) => {
                    _ = match stream.play() {
                        Ok(_) => ret.send(ControlInvocationResult::Ok(())),
                        Err(e) => ret.send(ControlInvocationResult::Err(e.to_string())),
                    }
                    .inspect_err(|e| log::error!("error sending resume ack: {e}"));
                }
                Control::Shutdown(ret) => {
                    drop(stream);
                    let _ = ret.send(ControlInvocationResult::Ok(()));
                    break;
                }
            }
        }
        // Thread exits; stream dropped.
    });

    // Await init result.
    match init_rx.recv() {
        Ok(Ok(())) => Ok((tx, handle)),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(InstrumentError::StartFailed {
            detail: Some("output stream owner init channel closed".into()),
        }),
    }
}

/// Build output stream across supported sample formats, wiring a format-agnostic
/// callback that pulls stereo samples from the generator and writes them into
/// the interleaved device buffer.
fn run_output(
    device: &cpal::Device,
    config: &StreamConfig,
    default_cfg: &SupportedStreamConfig,
    channels: usize,
    mut next_sample: Box<GenType>,
) -> common::error::Result<Stream> {
    let err_cb = |err| log::error!("instrument playback stream error: {err}");

    let sample_rate = config.sample_rate.0;
    // (Instant delay, accumulated delay)
    let mut last_ts: Option<StreamInstant> = None;

    match default_cfg.sample_format() {
        SampleFormat::F32 => device.build_output_stream(
            config,
            move |data: &mut [f32], info: &cpal::OutputCallbackInfo| {
                let is_late =
                    is_running_late(info, &mut last_ts, data.len() / channels, sample_rate);
                write_data(data, channels, &mut next_sample, is_late)
            },
            err_cb,
            Some(Duration::from_secs(STREAM_TIMEOUT_S)),
        ),
        SampleFormat::I16 => device.build_output_stream(
            config,
            move |data: &mut [i16], info: &cpal::OutputCallbackInfo| {
                let is_late =
                    is_running_late(info, &mut last_ts, data.len() / channels, sample_rate);
                write_data(data, channels, &mut next_sample, is_late)
            },
            err_cb,
            Some(Duration::from_secs(STREAM_TIMEOUT_S)),
        ),
        SampleFormat::U16 => device.build_output_stream(
            config,
            move |data: &mut [u16], info: &cpal::OutputCallbackInfo| {
                let is_late =
                    is_running_late(info, &mut last_ts, data.len() / channels, sample_rate);
                write_data(data, channels, &mut next_sample, is_late)
            },
            err_cb,
            Some(Duration::from_secs(STREAM_TIMEOUT_S)),
        ),
        other => {
            return Err(InstrumentError::UnsupportedSampleFormat(format!(
                "unsupported_sample_format:{other:?}"
            ))
            .into())
        }
    }
    .map_err(|e| {
        InstrumentError::BuildStream {
            detail: e.to_string(),
        }
        .into()
    })
}

fn is_running_late(
    info: &cpal::OutputCallbackInfo,
    last: &mut Option<StreamInstant>,
    frames: usize,
    sample_rate: u32,
) -> bool {
    let buffer_s: f64 = frames as f64 / sample_rate as f64;

    let threshold = Duration::from_secs_f64(buffer_s * 1.75);

    let ts = info.timestamp().callback;

    let last_timeline = last.get_or_insert(ts);

    let has_delay = ts
        .duration_since(last_timeline)
        .filter(|delay| delay > &threshold)
        .is_some();

    *last_timeline = ts;

    has_delay
}

/// Interleave generated stereo frames across the output slice.
///
/// If `channels` > 2, the pattern (L,R, 0,0,...) is repeated (simple spread).
fn write_data<T>(output: &mut [T], channels: usize, next_sample: &mut GenType, prefer_batch: bool)
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    if channels == 0 || output.is_empty() {
        return;
    }

    if !prefer_batch {
        write_interleaved(output.chunks_mut(channels).map(|f| {
            let d = next_sample(None).unwrap();
            (f, d)
        }));
    } else {
        const MAX_BATCH_FRAMES: usize = 1024;
        let mut scratch_left = [0_f32; MAX_BATCH_FRAMES];
        let mut scratch_right = [0_f32; MAX_BATCH_FRAMES];

        for chunk in output.chunks_mut(MAX_BATCH_FRAMES * channels) {
            let frames_in_chunk = chunk.len() / channels; // actual frame count for this chunk (<= 1024)

            // Slice scratch to the exact number of frames we will generate/write.
            let mut frames_per_channel = [
                &mut scratch_left[..frames_in_chunk],
                &mut scratch_right[..frames_in_chunk],
            ];

            // Ask generator for exactly frames_in_chunk frames.
            _ = next_sample(Some((frames_per_channel.as_mut_slice(), frames_in_chunk)));

            // Interleave only the produced frames.
            write_interleaved(
                chunk.chunks_mut(channels).zip(
                    scratch_left[..frames_in_chunk]
                        .iter()
                        .copied()
                        .zip(scratch_right[..frames_in_chunk].iter().copied()),
                ),
            );
        }
    }
}

fn write_interleaved<'a, I, T>(data: I)
where
    I: Iterator<Item = (&'a mut [T], (f32, f32))>,
    T: cpal::SizedSample + cpal::FromSample<f32> + 'a,
{
    for (frame, (l, r)) in data {
        let left: T = T::from_sample(l);

        if frame.len() == 1 {
            frame[0] = left;
            continue;
        }

        let right: T = T::from_sample(r);
        frame[0] = left;
        frame[1] = right;

        // Fill any additional channels with silence (or could duplicate).
        let zero = T::from_sample(0.0);
        for slot in frame.iter_mut().skip(2) {
            *slot = zero;
        }
    }
}
