mod commands;
mod error;
#[cfg(target_os = "macos")]
mod setup_mac_window;

use tauri::{App, Manager};

pub use commands::*;

pub fn app_setup(app: &mut App) -> Result<(), error::WindowError> {
    app.manage(commands::WindowAppearanceState::default());

    if let Some(mut main_window) = app.get_webview_window("main") {
        #[cfg(target_os = "macos")]
        if let Err(error) = setup_mac_window::setup(&mut main_window) {
            log::warn!("macOS window appearance setup failed: {error}");
        }

        main_window.on_window_event({
            let app_handle = app.app_handle().clone();
            move |event| {
                if matches!(event, tauri::WindowEvent::Destroyed) {
                    app_handle.exit(0);
                }
            }
        });
    } else {
        log::warn!("Main window not available during setup");
    }

    Ok(())
}
