use common::{
    commands::setup::UpdateWindowAppearanceOverridePayload,
    error::{Result, SetupError},
};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_store::StoreExt;

use super::WindowState;

#[tauri::command]
pub fn update_window_appearance(
    dark: bool,
    app: AppHandle,
    state: State<'_, WindowState>,
) -> Result<()> {
    log::debug!("Updating window appearance to dark mode: {}", dark);

    // Determine if override is set (narrow lock scope; drop before further work)
    let override_is_none = {
        let guard = state.lock();
        log::trace!("Current override_dark state: {:?}", guard.override_dark);
        guard.override_dark.is_none()
    };

    if override_is_none {
        log::debug!("No override set; applying appearance change");
        let mut main_window = app
            .get_webview_window("main")
            .ok_or(SetupError::MainWindowMissing)?;

        #[cfg(target_os = "macos")]
        {
            super::setup_mac_window::update_appearance(&mut main_window, dark)
                .map_err(SetupError::appearance)?;
        }

        {
            let mut guard = state.lock();
            guard.dark = dark;
        }

        app.emit(common::events::setup::UPDATE_WINDOW_APPEARANCE, ())
            .map_err(|e| SetupError::emit(common::events::setup::UPDATE_WINDOW_APPEARANCE, e))?;
    } else {
        log::trace!("Ignoring appearance update because an override is active");
    }

    Ok(())
}

#[tauri::command]
pub fn update_window_appearance_dark_override(
    dark: Option<bool>,
    app: AppHandle,
    state: State<'_, WindowState>,
) -> Result<()> {
    // 1. Update override flag (short lock scope)
    {
        let mut guard = state.lock();
        guard.override_dark = dark;
    }

    let store = app.store(super::SETUP_STORE_NAME).unwrap();
    // Persist the override (Option<bool>)
    store.set(super::DARK_OVERRIDE_KEY, dark);

    // Will hold the resolved effective dark value after applying logic
    let effective_dark: bool = if let Some(forced) = dark {
        // 2a. Override present: apply immediately
        let mut main_window = app
            .get_webview_window("main")
            .ok_or(SetupError::MainWindowMissing)?;
        #[cfg(target_os = "macos")]
        {
            super::setup_mac_window::update_appearance(&mut main_window, forced)
                .map_err(SetupError::appearance)?;
        }
        {
            let mut guard = state.lock();
            guard.dark = forced;
        }
        forced
    } else {
        // 2b. Override removed: recompute system appearance
        let mut main_window = app
            .get_webview_window("main")
            .ok_or(SetupError::MainWindowMissing)?;

        let system_dark = {
            #[cfg(target_os = "macos")]
            {
                super::setup_mac_window::setup(&mut main_window, None)
                    .map_err(SetupError::appearance)?
            }
            #[cfg(not(target_os = "macos"))]
            {
                false
            }
        };

        {
            let mut guard = state.lock();
            guard.dark = system_dark;
        }
        system_dark
    };

    // 3. Persist resolved (effective) dark value separately
    store.set("dark", Some(effective_dark));

    // 4. Emit override state payload (consumer can know if override active)
    app.emit(
        common::events::setup::GET_WINDOW_APPEARANCE_OVERRIDE,
        UpdateWindowAppearanceOverridePayload { dark },
    )
    .map_err(|e| SetupError::emit(common::events::setup::GET_WINDOW_APPEARANCE_OVERRIDE, e))?;

    // 5. Always emit appearance update so listeners react uniformly
    app.emit(common::events::setup::UPDATE_WINDOW_APPEARANCE, ())
        .map_err(|e| SetupError::emit(common::events::setup::UPDATE_WINDOW_APPEARANCE, e))?;

    Ok(())
}

#[tauri::command]
pub fn window_appearance_override(
    state: State<'_, WindowState>,
) -> Result<UpdateWindowAppearanceOverridePayload> {
    let guard = state.lock();
    Ok(UpdateWindowAppearanceOverridePayload {
        dark: guard.override_dark,
    })
}

#[tauri::command]
pub fn update_window_size(
    width: f64,
    height: f64,
    app: AppHandle,
    state: State<'_, WindowState>,
) -> Result<()> {
    {
        let mut guard = state.lock();
        guard.width = width;
        guard.height = height;
    }

    log::debug!("Updating window size to {}x{}", width, height);

    app.emit(common::events::setup::UPDATE_WINDOW_SIZE, ())
        .map_err(|e| SetupError::emit(common::events::setup::UPDATE_WINDOW_SIZE, e))?;

    Ok(())
}
