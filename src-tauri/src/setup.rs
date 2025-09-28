use tauri::{App, Manager};
#[cfg(not(any(target_os = "ios", target_os = "android")))]
use tauri_plugin_window_state::WindowExt;

pub fn app_setup(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let mut main_window = app.get_webview_window("main").ok_or("No main window")?;

    // Set background color only when building for macOS
    #[cfg(target_os = "macos")]
    {
        crate::setup_mac_window::setup(&mut main_window)?;
    }

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    main_window.restore_state(
        tauri_plugin_window_state::StateFlags::SIZE
            & tauri_plugin_window_state::StateFlags::POSITION,
    )?;

    Ok(())
}

#[tauri::command]
pub fn update_window_appearance(app: tauri::AppHandle, dark: bool) -> Result<(), String> {
    let mut main_window = app.get_webview_window("main").ok_or("No main window")?;

    #[cfg(target_os = "macos")]
    {
        crate::setup_mac_window::update_appearance(&mut main_window, dark)
            .map_err(|e| format!("Failed to update appearance: {}", e))?;
    }

    Ok(())
}
