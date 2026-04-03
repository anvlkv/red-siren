#![feature(duration_millis_float)]

mod app_bus;
mod health;
mod instrument;
mod intro;
mod persistence;
mod setup;
mod tuner;
mod windows;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();
    /*
     * ---------- Plugins ----------
     */

    // #[cfg(all(not(debug_assertions), not(feature = "devtools")))]
    // {
    //     builder = builder.plugin(tauri_plugin_prevent_default::init());
    // }
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
                log::LevelFilter::Debug
            } else if cfg!(feature = "devtools") {
                log::LevelFilter::Info
            } else {
                log::LevelFilter::Warn
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
        setup::go_back,
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
        tuner::tuner_update_range,
        tuner::tuner_update_threshold,
        tuner::tuner_update_wet_ratio,
        tuner::tuner_constraints_updated,
        tuner::tuner_snapshot_input_snoop,
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

        if let Err(e) = windows::ensure_startup_windows(app.handle()) {
            log::error!("failed to create startup windows: {}", e);
        }

        if let Err(e) = setup::app_setup(app) {
            log::error!("setup::app_setup failed: {}", e);
        }
        if let Err(e) = health::setup(app) {
            log::error!("health::setup failed: {}", e);
        }
        if let Err(e) = intro::setup(app) {
            log::error!("intro::setup failed: {}", e);
        }
        if let Err(e) = tuner::setup(app) {
            log::error!("tuner::setup failed: {}", e);
        }
        if let Err(e) = instrument::setup(app) {
            log::error!("instrument::setup failed: {}", e);
        }

        // Setup AppBus last
        if let Err(e) = app_bus::AppBus::setup(app) {
            log::error!("AppBus::setup failed: {}", e);
        }

        Ok(())
    });

    if let Err(e) = builder.run(tauri::generate_context!()) {
        log::error!("error while running tauri application: {}", e);
    }
}
