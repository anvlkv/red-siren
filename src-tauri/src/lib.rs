#![feature(duration_millis_float)]

mod health;
mod instrument;
mod intro;
mod setup;
mod tuner;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();
    /*
     * ---------- Plugins ----------
     */

    #[cfg(all(not(debug_assertions), not(feature = "devtools")))]
    {
        builder = builder.plugin(tauri_plugin_prevent_default::init());
    }
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        builder = builder.plugin(
            tauri_plugin_window_state::Builder::new()
                .skip_initial_state("splashscreen")
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::all()
                        & !tauri_plugin_window_state::StateFlags::VISIBLE,
                )
                .build(),
        );
    }
    builder = builder.plugin(tauri_plugin_store::Builder::new().build());
    builder = builder.plugin(
        tauri_plugin_log::Builder::new()
            .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
            .level(if cfg!(debug_assertions) {
                log::LevelFilter::Trace
            } else if cfg!(feature = "devtools") {
                log::LevelFilter::Debug
            } else {
                log::LevelFilter::Info
            })
            .target(tauri_plugin_log::Target::new(
                tauri_plugin_log::TargetKind::Webview,
            ))
            .build(),
    );
    builder = builder.plugin(tauri_plugin_opener::init());
    builder = builder.plugin(tauri_plugin_safe_area_insets_css::init());

    /*
     * ---------- Handlers ----------
     */

    builder = builder.invoke_handler(tauri::generate_handler![
        setup::update_window_appearance,
        setup::update_window_size,
        setup::update_window_appearance_dark_override,
        setup::window_appearance_override,
        setup::open_in_new_window,
        health::health_on_gui_ready,
        health::health_grant_mic_premission,
        health::health_setup_state,
        intro::intro_pause,
        intro::intro_resume,
        intro::intro_next_frame,
        tuner::tuner_config,
        tuner::tuner_layout,
        tuner::tuner_spectrum_data,
        tuner::tuner_update_sensor,
        tuner::tuner_reset_config,
        tuner::tuner_start_stream,
        tuner::tuner_stop_stream,
        tuner::tuner_toggle_probe,
        instrument::instrument_playback_start,
        instrument::instrument_playback_stop,
        instrument::instrument_playback_state,
        instrument::instrument_playback_pause,
        instrument::instrument_playback_resume,
        instrument::instrument_excitement_source,
        instrument::instrument_set_excitement_source,
        instrument::instrument_layout,
        instrument::ui_safe_area_insets_apply,
        instrument::instrument_string_snoop_data,
        instrument::instrument_all_string_snoops,
        instrument::instrument_excitement_snoop_data,
        instrument::instrument_all_excitement_snoops,
        instrument::instrument_update_band_control,
        instrument::instrument_update_key_control,
        instrument::instrument_quality_indicator,
        instrument::snapshot_processed_output_spectrum,
        #[cfg(feature = "devtools")]
        instrument::instrument_edit_finetuned_values,
        #[cfg(feature = "devtools")]
        instrument::instrument_get_finetuned_values,
    ]);

    // Setup logic

    builder = builder.setup(|app| {
        let config = app.config();
        log::debug!("App starting with config: {config:#?}");

        // Setup supporting modules
        setup::app_setup(app)?;
        health::setup(app)?;
        intro::setup(app)?;
        tuner::setup(app)?;

        // Setup instrument last
        instrument::setup(app)?;

        Ok(())
    });

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
