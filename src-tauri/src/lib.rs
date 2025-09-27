use crate::navigation::{new_manager, parse_incoming_route_payload};
use navigation_manager::NavigationManager;
use shared::RouteId;
use tauri::Manager;
use tokio::sync::Mutex;

mod health;
mod intro;
mod navigation;
mod navigation_manager;
mod setup;

#[cfg(target_os = "macos")]
mod setup_mac_window;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();
    // Register the setup state
    builder = builder.manage(Mutex::new(health::SetupState {
        gui_ready: false,
        backend_ready: false,
    }));
    // Intro engine state
    builder = builder.manage(intro::IntroEngineState::new());
    builder = builder.setup(|app| {
        // Existing setup logic
        setup::app_setup(app)?;
        // Initialize navigation manager with initial route (Home)
        let nav_manager = new_manager(app.handle().clone(), RouteId::Home);
        app.manage(nav_manager);
        Ok(())
    });
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
    builder = builder.plugin(tauri_plugin_log::Builder::new().build());
    builder = builder.plugin(tauri_plugin_opener::init());
    builder = builder.invoke_handler(tauri::generate_handler![
        health::health_on_gui_ready,
        navigation::navigation_request,
        navigation::navigation_leave_done,
        navigation::navigation_enter_done,
        navigation::navigation_sync,
        setup::update_window_appearance,
        intro::intro_pause,
        intro::intro_resume,
        intro::intro_next_frame,
    ]);
    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
