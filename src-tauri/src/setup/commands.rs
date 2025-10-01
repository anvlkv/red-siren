use tauri::{AppHandle, Emitter, Manager, State};
use shared::{commands::setup::UpdateWindowAppearanceOverridePayload, error::{Result, SetupError}};
use tauri_plugin_store::StoreExt;

use super::WindowState;

#[tauri::command]
pub async fn update_window_appearance(
    dark: bool,
    app: AppHandle,
    state: State<'_, WindowState>,
) -> Result<()> {
    let health_state = state.lock().await;

        if health_state.override_dark.is_none() {
            let mut main_window = app
                .get_webview_window("main")
                .ok_or(SetupError::MainWindowMissing)?;

            #[cfg(target_os = "macos")]
            {
                super::setup_mac_window::update_appearance(&mut main_window, dark)
                    .map_err(SetupError::appearance)?;
            }

            let mut state_lock = state.lock().await;
            state_lock.dark = dark;

            app.emit(shared::events::setup::UPDATE_WINDOW_APPEARANCE, ())
                .map_err(|e| SetupError::emit(shared::events::setup::UPDATE_WINDOW_APPEARANCE, e))?;
        }


    Ok(())
}

#[tauri::command]
pub async fn update_window_appearance_dark_override(
    dark: Option<bool>,
    app: AppHandle,
    state: State<'_, WindowState>,
) -> Result<()> {
    let mut health_state = state.lock().await;

        health_state.override_dark = dark;

        let store = app.store(super::SETUP_STORE_NAME).unwrap();

        store.set(super::DARK_OVERRIDE_KEY, dark);


        app.emit(shared::events::setup::GET_WINDOW_APPEARANCE_OVERRIDE, UpdateWindowAppearanceOverridePayload { dark })
            .map_err(|e| SetupError::emit(shared::events::setup::GET_WINDOW_APPEARANCE_OVERRIDE, e))?;

    if dark.is_none() {
        let mut main_window = app
            .get_webview_window("main")
            .ok_or(SetupError::MainWindowMissing)?;

        let dark = {
            #[cfg(target_os = "macos")]
            {
                super::setup_mac_window::setup(&mut main_window, None).map_err(SetupError::appearance)?
            }
            #[cfg(not(target_os = "macos"))]
            {
                false
            }
        };

        let mut state_lock = state.lock().await;
        state_lock.dark = dark;

        app.emit(shared::events::setup::UPDATE_WINDOW_APPEARANCE, ())
            .map_err(|e| SetupError::emit(shared::events::setup::UPDATE_WINDOW_APPEARANCE, e))?;
    }


    Ok(())
}

#[tauri::command]
pub async fn window_appearance_override(
    state: State<'_, WindowState>,
) -> Result<UpdateWindowAppearanceOverridePayload> {
    let health_state = state.lock().await;
    Ok(UpdateWindowAppearanceOverridePayload { dark: health_state.override_dark })
}

#[tauri::command]
pub async fn update_window_size(
    width: f64,
    height: f64,
    app: AppHandle,
    state: State<'_, WindowState>,
) -> Result<()> {
    let mut state_lock = state.lock().await;
    state_lock.width = width;
    state_lock.height = height;

    log::debug!("Updating window size to {}x{}", width, height);

    app.emit(shared::events::setup::UPDATE_WINDOW_SIZE, ())
        .map_err(|e| SetupError::emit(shared::events::setup::UPDATE_WINDOW_SIZE, e))?;

    Ok(())
}
