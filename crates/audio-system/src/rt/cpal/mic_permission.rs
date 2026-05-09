//! Microphone permission / availability check for the CPAL runtime.
//!
//! Strategy (MAYA DRY KISS):
//! - Attempt to open the default input device.
//! - Build a short–lived input stream with an empty data callback.
//! - Start (`play`) the stream, sleep briefly (~100ms), then drop it.
//! - If any CPAL operation fails (no device, config, build, play, or runtime
//!   error callback), return `HealthError::MicPermissionCheckFailed`.
//!
//! Rationale:
//! - This is a pragmatic probe to detect both permission denial and device
//!   unavailability without introducing heavy abstractions.
//! - The 100ms sleep is kept short to minimize blocking impact; the function
//!   is `async` for call-site symmetry, but internally performs a blocking
//!   operation (acceptable due to short duration).
//!
//! Future:
//! - If longer or more complex checks are needed, switch to spawning a
//!   dedicated blocking task (e.g. via `tokio::task::spawn_blocking`) once
//!   the runtime dependency is introduced to this crate.
//!
//! NOTE:
//! - We intentionally avoid adding a Tokio dependency here just for this
//!   probe. The blocking sleep is considered acceptable for now.
use common::error::HealthError;
use cpal::{
    Device, FromSample, InputCallbackInfo, SampleFormat, SizedSample, Stream, StreamConfig,
    StreamError,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use std::sync::Arc;
/// Perform the microphone permission / availability check for a concrete
/// sample format.
fn run<S>(device: &Device, config: &StreamConfig) -> Result<(), HealthError>
where
    S: SizedSample + FromSample<f32>,
{
    // Shared flag for error callback to store first error.
    let err_flag = Arc::new(std::sync::Mutex::new(None::<String>));
    // Clone for stream construction.
    let dev = device.clone();
    let cfg = config.clone();
    let err_flag_for_cb = err_flag.clone();
    // Empty data callback.
    let data_cb = move |_: &[S], _: &InputCallbackInfo| {};
    // Error callback writes exactly once into shared flag.
    let err_cb = move |err: StreamError| {
        if let Ok(mut guard) = err_flag_for_cb.lock()
            && guard.is_none()
        {
            *guard = Some(err.to_string());
        }
    };
    // Build input stream.
    let stream: Stream = dev
        .build_input_stream::<S, _, _>(
            &cfg,
            data_cb,
            err_cb,
            Some(std::time::Duration::from_secs(1)),
        )
        .map_err(|e| HealthError::MicPermissionCheckFailed {
            detail: Some(e.to_string()),
        })?;
    // Start stream.
    stream
        .play()
        .map_err(|e| HealthError::MicPermissionCheckFailed {
            detail: Some(e.to_string()),
        })?;
    // Briefly let callbacks run.
    std::thread::sleep(std::time::Duration::from_millis(100));
    // Drop stream before inspecting flag.
    drop(stream);
    if let Some(err) = err_flag.lock().ok().and_then(|mut g| g.take()) {
        return Err(HealthError::MicPermissionCheckFailed { detail: Some(err) });
    }
    Ok(())
}
/// Asynchronously (logically) check microphone permission / availability.
///
/// This function is `async` for ergonomic symmetry with callers that expect
/// an async API; internally it executes synchronously and returns immediately.
pub async fn check_mic_permission() -> Result<(), HealthError> {
    // Host
    let host = cpal::default_host();
    // Default input device.
    let device =
        host.default_input_device()
            .ok_or_else(|| HealthError::MicPermissionCheckFailed {
                detail: Some("no_input_device".into()),
            })?;
    // Default input config.
    let cfg = device
        .default_input_config()
        .map_err(|e| HealthError::MicPermissionCheckFailed {
            detail: Some(e.to_string()),
        })?;
    // Convert to owned StreamConfig.
    let stream_cfg: StreamConfig = cfg.clone().into();
    // Dispatch by sample format.
    match cfg.sample_format() {
        SampleFormat::F32 => run::<f32>(&device, &stream_cfg)?,
        SampleFormat::I16 => run::<i16>(&device, &stream_cfg)?,
        SampleFormat::U16 => run::<u16>(&device, &stream_cfg)?,
        other => {
            return Err(HealthError::MicPermissionCheckFailed {
                detail: Some(format!("unsupported_sample_format:{other:?}")),
            });
        }
    }
    Ok(())
}
