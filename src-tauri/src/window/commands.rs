use parking_lot::Mutex;
use tauri::{AppHandle, State, WebviewUrl, WebviewWindowBuilder};

use super::error::WindowError;

#[derive(Default)]
pub struct WindowAppearanceState {
    pub width: Mutex<u32>,
    pub height: Mutex<u32>,
    pub is_dark: Mutex<bool>,
}

#[tauri::command]
pub async fn update_window_appearance(
    width: u32,
    height: u32,
    is_dark: bool,
    state: State<'_, WindowAppearanceState>,
    window: tauri::Window,
) -> Result<(), WindowError> {
    if window.label() == crate::MAIN_WINDOW_LABEL {
        *state.width.lock() = width;
        *state.height.lock() = height;
        *state.is_dark.lock() = is_dark;

        log::debug!(
            "Updated main window appearance: width={width}, height={height}, is_dark={is_dark}",
        );
        Ok(())
    } else {
        Err(WindowError::SecondaryWindow(
            "update_window_appearance".to_string(),
        ))
    }
}

#[tauri::command]
pub async fn open_secondary_window(
    url: String,
    title: String,
    app: AppHandle,
    state: State<'_, WindowAppearanceState>,
) -> Result<(), WindowError> {
    let label = format!(
        "secondary:{}",
        url.trim_start_matches('/').replace('/', "-")
    );
    let width = *state.width.lock() as f64;
    let height = *state.height.lock() as f64;

    if app.supports_multiple_windows() {
        #[allow(unused_mut)]
        let mut builder = WebviewWindowBuilder::new(
            &app,
            label.as_str(),
            WebviewUrl::App(format!("{url}").into()),
        )
        .title(title)
        .decorations(true)
        .resizable(true)
        .maximizable(false)
        .minimizable(false)
        .maximized(false)
        .inner_size(width, height);

        #[cfg(target_os = "macos")]
        {
            builder = builder.title_bar_style(tauri::TitleBarStyle::Transparent);
        }

        builder
            .build()
            .map_err(|e| WindowError::WindowCreationError(e.to_string()))?;

        Ok(())
    } else {
        Err(WindowError::MultipleWindowsNotSupported)
    }
}
