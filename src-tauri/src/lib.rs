use tokio::sync::Mutex;

mod health;
mod setup;

#[cfg(target_os = "macos")]
mod setup_mac_window;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Register the setup state
        .manage(Mutex::new(health::SetupState {
            gui_ready: false,
            backend_ready: false,
        }))
        .setup(setup::app_setup)
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::all()
                        & !tauri_plugin_window_state::StateFlags::VISIBLE,
                )
                .build(),
        )
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![health::health_on_gui_ready])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
