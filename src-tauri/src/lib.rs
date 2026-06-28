mod audio_runtime;
mod dsp;
mod window;

pub const MAIN_WINDOW_LABEL: &str = "main";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();
    builder = builder.plugin(
        tauri_plugin_log::Builder::new()
            .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
            .level({
                let default = if cfg!(debug_assertions) {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Warn
                };
                std::env::var("RUST_LOG")
                    .ok()
                    .and_then(|v| v.parse::<log::LevelFilter>().ok())
                    .unwrap_or(default)
            })
            .target(tauri_plugin_log::Target::new(
                tauri_plugin_log::TargetKind::Webview,
            ))
            .build(),
    );
    builder = builder.plugin(tauri_plugin_opener::init());
    builder = builder.plugin(tauri_plugin_window_state::Builder::new().build());

    #[cfg(not(debug_assertions))]
    {
        builder = builder.plugin(tauri_plugin_prevent_default::init());
    }

    builder = builder.setup(|app| {
        let config = app.config();
        log::debug!("App starting with config: {config:#?}");

        window::app_setup(app)?;

        dsp::app_setup(app)?;

        audio_runtime::app_setup(app)?;

        Ok(())
    });

    builder = builder.invoke_handler(tauri::generate_handler![
        window::update_window_appearance,
        window::open_secondary_window,
        audio_runtime::start_playback,
        audio_runtime::stop_playback,
        audio_runtime::pause_playback,
        audio_runtime::resume_playback,
        audio_runtime::list_audio_devices,
        audio_runtime::select_input_device,
        audio_runtime::select_output_device,
        audio_runtime::set_quality,
        audio_runtime::get_playback_state,
        audio_runtime::get_current_quality,
        dsp::create_synth,
        dsp::set_speed,
        dsp::set_shape,
        dsp::set_window,
        dsp::get_snapshot,
    ]);

    if let Err(e) = builder.run(tauri::generate_context!()) {
        log::error!("error while running tauri application: {}", e);
        panic!("error while running tauri application: {}", e);
    }
}
