use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use common::error::InstrumentError;
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig, SupportedStreamConfig};

use super::{Control, ControlInvocationResult};

pub type GenType = dyn FnMut() -> (f32, f32) + Send;

/// Spawn an owner thread that creates and owns the CPAL stream.
/// The stream is created inside the owner thread and never moved across threads.
/// Returns a control Sender and the thread JoinHandle after successful initialization.
///
/// - `device`, `default_cfg`, `stream_cfg` are consumed and moved into the owner.
/// - `make_next` constructs the audio sample source within the owner thread to avoid cross-thread moves.
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
    // One-shot init channel so the caller learns whether stream creation succeeded.
    let (init_tx, init_rx) = mpsc::channel::<Result<(), InstrumentError>>();

    let handle = thread::spawn(move || {
        // Build audio generator inside the owner thread.
        let next = make_next();

        // Build the stream in this thread; never move it elsewhere.
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

        // Signal successful initialization.
        let _ = init_tx.send(Ok(()));

        // Control loop
        while let Ok(msg) = rx.recv() {
            match msg {
                Control::Pause(ret) => {
                    _ = match stream.pause() {
                        Ok(_) => ret.send(ControlInvocationResult::Ok(())),
                        Err(e) => ret.send(ControlInvocationResult::Err(e.to_string())),
                    }
                    .inspect_err(|e| {
                        log::error!("erro sending control result: {e}");
                    });
                }
                Control::Resume(ret) => {
                    _ = match stream.play() {
                        Ok(_) => ret.send(ControlInvocationResult::Ok(())),
                        Err(e) => ret.send(ControlInvocationResult::Err(e.to_string())),
                    }
                    .inspect_err(|e| {
                        log::error!("erro sending control result: {e}");
                    });
                }
                Control::Shutdown(ret) => {
                    drop(stream);
                    let _ = ret.send(ControlInvocationResult::Ok(()));
                    break;
                }
            }
        }
    });

    // Wait for init result from the owner thread.
    match init_rx.recv() {
        Ok(Ok(())) => Ok((tx, handle)),
        Ok(Err(e)) => {
            // Best-effort: the thread should have exited; if not, it's detached.
            Err(e)
        }
        Err(_) => Err(InstrumentError::StartFailed {
            detail: Some("stream owner init channel closed".into()),
        }),
    }
}

/// Build output stream across supported sample formats.
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
                log::trace!("output stream tick: {} samples", data.len());
                write_data(data, channels, &mut next_sample)
            },
            err_cb,
            Some(Duration::from_secs(super::STREAM_TIMEOUT_S)),
        ),
        SampleFormat::I16 => device.build_output_stream(
            config,
            move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                log::trace!("output stream tick: {} samples", data.len());
                write_data(data, channels, &mut next_sample)
            },
            err_cb,
            Some(Duration::from_secs(super::STREAM_TIMEOUT_S)),
        ),
        SampleFormat::U16 => device.build_output_stream(
            config,
            move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                log::trace!("output stream tick: {} samples", data.len());
                write_data(data, channels, &mut next_sample)
            },
            err_cb,
            Some(Duration::from_secs(super::STREAM_TIMEOUT_S)),
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

/// Interleave stereo frames into the output buffer using next_sample.
fn write_data<T>(output: &mut [T], channels: usize, next_sample: &mut GenType)
where
    T: cpal::SizedSample + cpal::FromSample<f64>,
{
    for frame in output.chunks_mut(channels) {
        let (l, r) = next_sample();
        log::trace!("next sample: L={l}, R={r}");

        let left: T = T::from_sample(l as f64);
        let right: T = T::from_sample(r as f64);

        for (channel, sample) in frame.iter_mut().enumerate() {
            if channel & 1 == 0 {
                *sample = left;
            } else {
                *sample = right;
            }
        }
    }
}
