mod setup;

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

    builder = builder.setup(|app| {
        let config = app.config();
        log::debug!("App starting with config: {config:#?}");

        if let Err(e) = setup::app_setup(app) {
            log::error!("setup::app_setup failed: {}", e);
        }

        Ok(())
    });

    if let Err(e) = builder.run(tauri::generate_context!()) {
        log::error!("error while running tauri application: {}", e);
    }
}
