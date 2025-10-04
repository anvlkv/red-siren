use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig, SupportedStreamConfig};

use common::error::InstrumentError;

/// Result of a control invocation, sent back to the caller per request.
#[derive(Debug)]
pub enum ControlInvocationResult {
    OkChanged,
    NoOp,
    BackendMissing,
    Error(String),
}

/// Control messages for the stream owner.
pub enum Control {
    Pause(Sender<ControlInvocationResult>),
    Resume(Sender<ControlInvocationResult>),
    Shutdown(Sender<ControlInvocationResult>),
}

/// Spawn an owner thread that creates and owns the CPAL stream.
/// The stream is created inside the owner thread and never moved across threads.
/// Returns a control Sender and the thread JoinHandle after successful initialization.
///
/// - `device`, `default_cfg`, `stream_cfg` are consumed and moved into the owner.
/// - `make_next` constructs the audio sample source within the owner thread to avoid cross-thread moves.
pub fn spawn_owner<FMake>(
    device: cpal::Device,
    default_cfg: SupportedStreamConfig,
    stream_cfg: StreamConfig,
    make_next: FMake,
) -> Result<(Sender<Control>, thread::JoinHandle<()>), InstrumentError>
where
    FMake: Send + 'static + FnOnce() -> Box<dyn FnMut() -> (f32, f32) + Send>,
{
    let (tx, rx): (Sender<Control>, Receiver<Control>) = mpsc::channel();
    // One-shot init channel so the caller learns whether stream creation succeeded.
    let (init_tx, init_rx) = mpsc::channel::<Result<(), InstrumentError>>();

    let handle = thread::spawn(move || {
        // Build audio generator inside the owner thread.
        let next = make_next();

        // Build the stream in this thread; never move it elsewhere.
        let stream = match run(&device, &stream_cfg, &default_cfg, next) {
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
                    match stream.pause() {
                        Ok(_) => ret.send(ControlInvocationResult::OkChanged),
                        Err(e) => ret.send(ControlInvocationResult::Error(e.to_string())),
                    }
                    .ok();
                }
                Control::Resume(ret) => {
                    match stream.play() {
                        Ok(_) => ret.send(ControlInvocationResult::OkChanged),
                        Err(e) => ret.send(ControlInvocationResult::Error(e.to_string())),
                    }
                    .ok();
                }
                Control::Shutdown(ret) => {
                    drop(stream);
                    let _ = ret.send(ControlInvocationResult::OkChanged);
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

/// Moved from engine.rs (L172-178):
/// Build output stream across supported sample formats.
pub fn run(
    device: &cpal::Device,
    config: &StreamConfig,
    default_cfg: &SupportedStreamConfig,
    mut next_sample: Box<dyn FnMut() -> (f32, f32) + Send>,
) -> common::error::Result<Stream> {
    let err_cb = |err| log::error!("instrument playback stream error: {err}");

    match default_cfg.sample_format() {
        SampleFormat::F32 => device.build_output_stream(
            config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                write_data(data, 2, &mut next_sample)
            },
            err_cb,
            None,
        ),
        SampleFormat::I16 => device.build_output_stream(
            config,
            move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                write_data(data, 2, &mut next_sample)
            },
            err_cb,
            None,
        ),
        SampleFormat::U16 => device.build_output_stream(
            config,
            move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                write_data(data, 2, &mut next_sample)
            },
            err_cb,
            None,
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

/// Moved from engine.rs (L223-224):
/// Interleave stereo frames into the output buffer using next_sample.
pub fn write_data<T>(output: &mut [T], channels: usize, next_sample: &mut dyn FnMut() -> (f32, f32))
where
    T: cpal::SizedSample + cpal::FromSample<f64>,
{
    for frame in output.chunks_mut(channels) {
        let (l, r) = next_sample();
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
