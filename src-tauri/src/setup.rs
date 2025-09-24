use tauri::{async_runtime::spawn, App, Manager};
use tauri_plugin_window_state::WindowExt;
use tokio::{sync::Mutex, time::Duration};

use crate::health;

pub fn app_setup(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let mut main_window = app.get_webview_window("main").ok_or("No main window")?;

    // Set background color only when building for macOS
    #[cfg(target_os = "macos")]
    {
        crate::setup_mac_window::setup(&mut main_window)?;
    }

    main_window.restore_state(tauri_plugin_window_state::StateFlags::SIZE & tauri_plugin_window_state::StateFlags::POSITION)?;

    // Start backend setup as an async task
    let app_handle = app.handle().clone();
    spawn(async move {
        // Wait a bit for the app to fully initialize
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Get state from app handle
        let app_handle_clone = app_handle.clone();
        if let Some(state) = app_handle.try_state::<Mutex<health::SetupState>>() {
            health::perform_backend_setup(app_handle_clone, state.inner()).await.ok();
        }
    });

    Ok(())
}
