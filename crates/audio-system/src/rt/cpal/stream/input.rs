use std::{
    sync::mpsc::{channel, Receiver, Sender},
    thread,
    time::Duration,
};

use common::error::InstrumentError;
use cpal::{
    traits::{DeviceTrait, StreamTrait},
    SampleFormat, Stream, StreamConfig, SupportedStreamConfig,
};

use crate::util::S;

use super::{Control, ControlInvocationResult, STREAM_TIMEOUT_S};

/// Producer callback type used by the input stream owner thread.
/// It receives a slice of f64 mono samples (already down-mixed) and
/// returns the number of samples successfully pushed into its buffer.
pub type ProdType = dyn FnMut(&S) + Send;

/// Spawn a dedicated owner thread that:
/// - Builds and owns the CPAL input stream (kept on that thread).
/// - Down-mixes multi-channel input to mono.
/// - Feeds mono samples into a user-supplied producer closure.
/// - Responds to control messages (Pause / Resume / Shutdown) with ACK.
///
/// Returns:
/// - `Sender<Control>` to issue control commands.
/// - `JoinHandle<()>` of the spawned thread.
///
/// Design (MAYA DRY KISS):
/// - Keep complex CPAL + buffer lifetime constraints encapsulated.
/// - Avoid moving the CPAL `Stream` across threads (creation + control on owner).
/// - Provide explicit ACK channel for deterministic control semantics.
pub fn spawn_owned_input_stream<FProd>(
    device: cpal::Device,
    default_cfg: SupportedStreamConfig,
    stream_cfg: StreamConfig,
    make_prod: FProd,
) -> Result<(Sender<Control>, thread::JoinHandle<()>), InstrumentError>
where
    FProd: Send + 'static + FnOnce() -> Box<ProdType>,
{
    let (tx, rx): (Sender<Control>, Receiver<Control>) = channel();
    // One-shot init channel so caller learns whether stream creation succeeded.
    let (init_tx, init_rx) = channel::<Result<(), InstrumentError>>();

    let handle = thread::spawn(move || {
        // Construct producer on the owner thread.
        let prod = make_prod();

        // Build CPAL stream on this thread; never move it afterwards.
        let stream = match run_input(&device, &stream_cfg, &default_cfg, prod) {
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
                    .inspect_err(|e| log::error!("error sending control result: {e}"));
                }
                Control::Resume(ret) => {
                    _ = match stream.play() {
                        Ok(_) => ret.send(ControlInvocationResult::Ok(())),
                        Err(e) => ret.send(ControlInvocationResult::Err(e.to_string())),
                    }
                    .inspect_err(|e| log::error!("error sending control result: {e}"));
                }
                Control::Shutdown(ret) => {
                    drop(stream);
                    let _ = ret.send(ControlInvocationResult::Ok(()));
                    break;
                }
            }
        }
    });

    // Await initialization result from owner thread.
    match init_rx.recv() {
        Ok(Ok(())) => Ok((tx, handle)),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(InstrumentError::StartFailed {
            detail: Some("stream owner init channel closed".into()),
        }),
    }
}

/// Build an input stream across supported sample formats.
///
/// The returned stream invokes a data callback which:
/// 1. Copies & converts raw samples into a scratch buffer (f64).
/// 2. Down-mixes multi-channel frames to mono (averaging).
/// 3. Passes mono slice to the provided producer closure.
fn run_input(
    device: &cpal::Device,
    config: &StreamConfig,
    default_cfg: &SupportedStreamConfig,
    mut produce_sample: Box<ProdType>,
) -> common::error::Result<Stream> {
    let err_cb = |err| log::error!("instrument input stream error: {err}");
    let channels = default_cfg.channels() as usize;

    match default_cfg.sample_format() {
        SampleFormat::F32 => device.build_input_stream(
            config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                log::trace!("input stream callback: received {} f32 samples", data.len());
                write_data(data, channels, &mut produce_sample)
            },
            err_cb,
            Some(Duration::from_secs(STREAM_TIMEOUT_S)),
        ),
        SampleFormat::I16 => device.build_input_stream(
            config,
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                log::trace!("input stream callback: received {} i16 samples", data.len());
                write_data(data, channels, &mut produce_sample)
            },
            err_cb,
            Some(Duration::from_secs(STREAM_TIMEOUT_S)),
        ),
        SampleFormat::U16 => device.build_input_stream(
            config,
            move |data: &[u16], _: &cpal::InputCallbackInfo| {
                log::trace!("input stream callback: received {} u16 samples", data.len());
                write_data(data, channels, &mut produce_sample)
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

/// Convert & down-mix frames to mono and feed producer.
fn write_data<T>(input: &[T], channels: usize, produce_sample: &mut ProdType)
where
    T: cpal::SizedSample + dasp_sample::ToSample<S>,
{
    for frame in input.chunks(channels) {
        let sample = (0..channels).map(|i| frame[i].to_sample()).sum::<S>() / channels as S;
        produce_sample(&sample);
    }
}
