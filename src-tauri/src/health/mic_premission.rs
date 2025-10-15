use common::error::HealthError;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    *,
};
use std::sync::Arc;

async fn run<S>(device: &Device, config: &StreamConfig) -> std::result::Result<(), HealthError>
    where
        S: SizedSample + FromSample<f32>,
    {
        // Shared place for the error callback to deposit any stream error.
        let err_flag = Arc::new(std::sync::Mutex::new(None::<String>));

        // Clone items we need to move into the blocking closure.
        let device = device.clone();
        let config = config.clone();
        let err_flag_for_thread = err_flag.clone();

        // Run the stream lifecycle on a blocking thread so non-Send callbacks/stream
        // do not live across .await points on the async runtime.
        let join_handle = tokio::task::spawn_blocking(move || -> std::result::Result<(), HealthError> {
            // Data callback — intentionally empty.
            let data_callback = move |_: &[S], _: &InputCallbackInfo| {};

            // Error callback writes into the shared flag.
            let err_flag_for_cb = err_flag_for_thread.clone();
            let err_callback = move |err: StreamError| {
                if let Ok(mut guard) = err_flag_for_cb.lock() {
                    if guard.is_none() {
                        *guard = Some(format!("Stream error: {err}"));
                    }
                }
            };

            // Build stream (map CPAL errors).
            let stream = device
                .build_input_stream::<S, _, _>(
                    &config,
                    data_callback,
                    err_callback,
                    Some(std::time::Duration::from_secs(1)),
                )
                .map_err(|e| HealthError::MicPermissionCheckFailed { detail: Some(e.to_string()) })?;

            stream
                .play()
                .map_err(|e| HealthError::MicPermissionCheckFailed { detail: Some(e.to_string()) })?;

            // Sleep on the blocking thread so we don't block the async reactor.
            std::thread::sleep(std::time::Duration::from_millis(100));

            // Drop stream before inspecting error flag.
            drop(stream);

            // Check if the callback reported an error.
            if let Some(err_str) = err_flag_for_thread.lock().unwrap().take() {
                return Err(HealthError::MicPermissionCheckFailed { detail: Some(err_str) });
            }

            Ok(())
        });

        // Await the blocking task's result on the async runtime.
        match join_handle.await {
            Ok(inner) => inner,
            Err(e) => Err(HealthError::MicPermissionCheckFailed { detail: Some(e.to_string()) }),
        }
    }

pub async fn check() -> std::result::Result<(), HealthError> {
    let host = default_host();

    // Handle lack of default input device explicitly.
    let device = match host.default_input_device() {
        Some(d) => d,
        None => return Err(HealthError::MicPermissionCheckFailed { detail: Some("no_input_device".into()) }),
    };

    let cfg = device
        .default_input_config()
        .map_err(|e| HealthError::MicPermissionCheckFailed { detail: Some(e.to_string()) })?;

    // Convert to concrete StreamConfig to avoid borrowing a temporary.
    let stream_cfg: StreamConfig = cfg.clone().into();

    match cfg.sample_format() {
        SampleFormat::F32 => run::<f32>(&device, &stream_cfg).await?,
        SampleFormat::I16 => run::<i16>(&device, &stream_cfg).await?,
        SampleFormat::U16 => run::<u16>(&device, &stream_cfg).await?,
        f => return Err(HealthError::MicPermissionCheckFailed { detail: Some(format!("unsupported_sample_format:{f:?}")) }),
    }

    Ok(())
}
