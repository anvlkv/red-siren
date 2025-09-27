use tauri::{async_runtime::spawn, App, Manager};
use tokio::{sync::Mutex};
#[cfg(not(any(target_os = "ios", target_os = "android")))]
use tauri_plugin_window_state::WindowExt;

use crate::health;

pub fn app_setup(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let mut main_window = app.get_webview_window("main").ok_or("No main window")?;

    // Set background color only when building for macOS
    #[cfg(target_os = "macos")]
    {
        crate::setup_mac_window::setup(&mut main_window)?;
    }

    #[cfg(not(any(target_os="ios", target_os="android")))]
    main_window.restore_state(tauri_plugin_window_state::StateFlags::SIZE & tauri_plugin_window_state::StateFlags::POSITION)?;

    // Start backend setup as an async task
    let app_handle = app.handle().clone();
    spawn(async move {
        // Wait a bit for the app to fully initialize
        // tokio::time::sleep(Duration::from_millis(100)).await;

        // Get state from app handle
        let app_handle_clone = app_handle.clone();
        if let Some(state) = app_handle.try_state::<Mutex<health::SetupState>>() {
            health::perform_backend_setup(app_handle_clone, state.inner()).await.ok();
        }
    });

    Ok(())
}

#[tauri::command]
pub fn update_window_appearance(
    app: tauri::AppHandle,
    dark: bool,
) -> Result<(), String> {
    let mut main_window = app.get_webview_window("main").ok_or("No main window")?;

    #[cfg(target_os = "macos")]
    {
        crate::setup_mac_window::update_appearance(&mut main_window, dark)
            .map_err(|e| format!("Failed to update appearance: {}", e))?;
    }

    Ok(())
}
