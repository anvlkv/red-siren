mod health;
mod instrument;
mod intro;
mod navigation;
mod setup;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    builder = builder.setup(|app| {
        let config = app.config();
        log::debug!("App starting with config: {config:#?}");

        // Existing setup logic
        setup::app_setup(app)?;
        navigation::setup(app)?;
        health::setup(app)?;
        intro::setup(app)?;
        instrument::setup(app)?;

        Ok(())
    });

    /*
     * ---------- Plugins ----------
     */

    #[cfg(not(debug_assertions))]
    {
        builder = builder.plugin(tauri_plugin_prevent_default::init());
    }
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        builder = builder.plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::all()
                        & !tauri_plugin_window_state::StateFlags::VISIBLE,
                )
                .build(),
        );
    }
    builder = builder.plugin(tauri_plugin_store::Builder::new().build());
    builder = builder.plugin(tauri_plugin_log::Builder::new().build());
    builder = builder.plugin(tauri_plugin_opener::init());

    /*
     * ---------- Handlers ----------
     */

    builder = builder.invoke_handler(tauri::generate_handler![
        setup::update_window_appearance,
        health::health_on_gui_ready,
        health::health_grant_mic_premission,
        navigation::navigation_bootstrap,
        navigation::navigation_request,
        navigation::navigation_leave_done,
        navigation::navigation_enter_done,
        navigation::navigation_sync,
        navigation::navigation_resume,
        intro::intro_pause,
        intro::intro_resume,
        intro::intro_next_frame,
        instrument::instrument_playback_start,
        instrument::instrument_playback_stop,
        instrument::instrument_playback_state,
        instrument::instrument_playback_pause,
        instrument::instrument_playback_resume,
    ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
