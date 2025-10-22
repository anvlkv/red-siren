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
    SampleFormat, Stream, StreamConfig, SupportedStreamConfig,
};

use super::{Control, ControlInvocationResult, STREAM_TIMEOUT_S};

/// Stereo (L,R) sample generator invoked per audio frame.
pub type GenType = dyn FnMut() -> (f32, f32) + Send;

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

    match default_cfg.sample_format() {
        SampleFormat::F32 => device.build_output_stream(
            config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                write_data(data, channels, &mut next_sample)
            },
            err_cb,
            Some(Duration::from_secs(STREAM_TIMEOUT_S)),
        ),
        SampleFormat::I16 => device.build_output_stream(
            config,
            move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                write_data(data, channels, &mut next_sample)
            },
            err_cb,
            Some(Duration::from_secs(STREAM_TIMEOUT_S)),
        ),
        SampleFormat::U16 => device.build_output_stream(
            config,
            move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                write_data(data, channels, &mut next_sample)
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

/// Interleave generated stereo frames across the output slice.
///
/// If `channels` > 2, the pattern (L,R, 0,0,...) is repeated (simple spread).
fn write_data<T>(output: &mut [T], channels: usize, next_sample: &mut GenType)
where
    T: cpal::SizedSample + cpal::FromSample<f64>,
{
    if channels == 0 {
        return;
    }
    for frame in output.chunks_mut(channels) {
        let (l, r) = next_sample();
        let left: T = T::from_sample(l as f64);

        if frame.len() == 1 {
            frame[0] = left;
            continue;
        }

        let right: T = T::from_sample(r as f64);
        frame[0] = left;
        frame[1] = right;

        // Fill any additional channels with silence (or could duplicate).
        let zero = T::from_sample(0.0f64);
        for slot in frame.iter_mut().skip(2) {
            *slot = zero;
        }
    }
}
