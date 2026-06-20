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

use cpal::{
    traits::{DeviceTrait, StreamTrait},
    OutputStreamTimestamp, SampleFormat, Stream, StreamConfig, SupportedStreamConfig,
};

use super::{AudioStreamError, Control, ControlInvocationResult, STREAM_TIMEOUT_S};

/// Stereo (L,R) sample generator invoked per audio frame or with batch.
pub type GenType = dyn FnMut(OutputStreamTimestamp, &mut [&mut [f32]]) + Send + Sync;

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
/// - Stream creation / play failures surfaced as `AudioStreamError`.
pub fn spawn_owned_output_stream<FMake>(
    device: cpal::Device,
    default_cfg: SupportedStreamConfig,
    stream_cfg: StreamConfig,
    channels: usize,
    make_next: FMake,
) -> Result<(Sender<Control>, thread::JoinHandle<()>), AudioStreamError>
where
    FMake: Send + 'static + FnOnce() -> Box<GenType>,
{
    let (tx, rx): (Sender<Control>, Receiver<Control>) = mpsc::channel();
    let (init_tx, init_rx) = mpsc::channel::<Result<(), AudioStreamError>>();

    let handle = thread::spawn(move || {
        // Build generator inside owner thread to keep its captures thread-affine if needed.
        let next = make_next();

        // Build CPAL stream on this thread.
        let mut stream_opt: Option<Stream> =
            match run_output(&device, &stream_cfg, &default_cfg, channels, next) {
                Ok(s) => Some(s),
                Err(e) => {
                    let _ = init_tx.send(Err(AudioStreamError::Startup(e.to_string())));
                    return;
                }
            };

        if let Some(ref s) = stream_opt {
            if let Err(e) = s.play() {
                let _ = init_tx.send(Err(AudioStreamError::Startup(e.to_string())));
                return;
            }
        } else {
            let _ = init_tx.send(Err(AudioStreamError::Startup("no stream available".into())));
            return;
        }

        // Signal success.
        let _ = init_tx.send(Ok(()));

        log::debug!("Output stream owner thread started, entering control loop");
        // Control loop (blocks on receiver, keeping stream alive).
        while let Ok(msg) = rx.recv() {
            match msg {
                Control::Pause(ret) => {
                    _ = match stream_opt.as_ref().unwrap().pause() {
                        Ok(_) => ret.send(ControlInvocationResult::Ok(())),
                        Err(e) => ret.send(ControlInvocationResult::Err(e.to_string())),
                    }
                    .inspect_err(|e| log::error!("error sending pause ack: {e}"));
                }
                Control::Resume(ret) => {
                    _ = match stream_opt.as_ref().unwrap().play() {
                        Ok(_) => ret.send(ControlInvocationResult::Ok(())),
                        Err(e) => ret.send(ControlInvocationResult::Err(e.to_string())),
                    }
                    .inspect_err(|e| log::error!("error sending resume ack: {e}"));
                }
                Control::Shutdown(ret) => {
                    let _ = stream_opt.take();
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
        Err(_) => Err(AudioStreamError::Startup(
            "output stream owner init channel closed".into(),
        )),
    }
}

/// Build output stream across supported sample formats, wiring a format-agnostic
/// callback that pulls stereo samples from the generator and writes them into
/// the interleaved device buffer.
fn run_output(
    device: &cpal::Device,
    &config: &StreamConfig,
    default_cfg: &SupportedStreamConfig,
    channels: usize,
    mut next_tick: Box<GenType>,
) -> Result<Stream, AudioStreamError> {
    let err_cb = |err| log::error!("instrument playback stream error: {err}");

    let mut scratch_left: Vec<f32> = Vec::new();
    let mut scratch_right: Vec<f32> = Vec::new();

    match default_cfg.sample_format() {
        SampleFormat::F32 => device.build_output_stream(
            config,
            move |data: &mut [f32], info: &cpal::OutputCallbackInfo| {
                write_data(
                    data,
                    channels,
                    info.timestamp(),
                    &mut next_tick,
                    &mut scratch_left,
                    &mut scratch_right,
                )
            },
            err_cb,
            Some(Duration::from_secs(STREAM_TIMEOUT_S)),
        ),
        SampleFormat::I16 => device.build_output_stream(
            config,
            move |data: &mut [i16], info: &cpal::OutputCallbackInfo| {
                write_data(
                    data,
                    channels,
                    info.timestamp(),
                    &mut next_tick,
                    &mut scratch_left,
                    &mut scratch_right,
                )
            },
            err_cb,
            Some(Duration::from_secs(STREAM_TIMEOUT_S)),
        ),
        SampleFormat::U16 => device.build_output_stream(
            config,
            move |data: &mut [u16], info: &cpal::OutputCallbackInfo| {
                write_data(
                    data,
                    channels,
                    info.timestamp(),
                    &mut next_tick,
                    &mut scratch_left,
                    &mut scratch_right,
                )
            },
            err_cb,
            Some(Duration::from_secs(STREAM_TIMEOUT_S)),
        ),
        other => {
            return Err(AudioStreamError::UnsupportedSampleFormat(format!(
                "unsupported_sample_format:{other:?}"
            )));
        }
    }
    .map_err(|e| AudioStreamError::BuildStream(e.to_string()))
}

/// Interleave generated stereo frames across the output slice.
///
/// If `channels` > 2, the pattern (L,R, 0,0,...) is repeated (simple spread).
fn write_data<T>(
    output: &mut [T],
    channels: usize,
    timestamp: OutputStreamTimestamp,
    next_tick: &mut GenType,
    scratch_left: &mut Vec<f32>,
    scratch_right: &mut Vec<f32>,
) where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    if channels == 0 || output.is_empty() {
        return;
    }

    let len = output.len() / channels;

    if scratch_left.len() != len {
        scratch_left.resize(len, 0.0);
    }
    if scratch_right.len() != len {
        scratch_right.resize(len, 0.0);
    }

    let mut frames_per_channel = [scratch_left.as_mut_slice(), scratch_right.as_mut_slice()];

    next_tick(timestamp, frames_per_channel.as_mut_slice());

    // Interleave the produced frames.
    write_interleaved(
        output.chunks_mut(channels).zip(
            scratch_left
                .iter()
                .copied()
                .zip(scratch_right.iter().copied()),
        ),
    );
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

        // Fill any additional channels with silence.
        let zero = T::from_sample(0.0);
        for slot in frame.iter_mut().skip(2) {
            *slot = zero;
        }
    }
}
