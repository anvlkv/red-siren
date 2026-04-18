use crate::persistence::persistence::save_json;
use crate::windows;
use common::{
    commands::setup::UpdateWindowAppearanceOverridePayload,
    error::{Result, SetupError},
    RouteId,
};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

use super::WindowState;

#[tauri::command]
pub fn update_window_appearance(
    dark: bool,
    app: AppHandle,
    state: State<'_, WindowState>,
    window: tauri::Window,
) -> Result<()> {
    if window.label() != "main" {
        return Ok(());
    }
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

    // Persist the override (Option<bool>) via persistence helper
    if let Err(e) = save_json(
        &app,
        super::SETUP_STORE_NAME,
        super::DARK_OVERRIDE_KEY,
        &dark,
    ) {
        log::error!(
            "Failed to persist window appearance override to store {} key {}: {}",
            super::SETUP_STORE_NAME,
            super::DARK_OVERRIDE_KEY,
            e
        );
    }

    let mut system_dark = false;

    // Will hold the resolved effective dark value after applying logic
    #[cfg(target_os = "macos")]
    for (_, window) in app.webview_windows().iter_mut() {
        if let Some(forced) = dark {
            super::setup_mac_window::update_appearance(window, forced)
                .map_err(SetupError::appearance)?;
        } else {
            system_dark =
                super::setup_mac_window::setup(window, None).map_err(SetupError::appearance)?;
        };
    }

    if let Some(forced) = dark {
        let mut guard = state.lock();
        guard.dark = forced;
    } else {
        let mut guard = state.lock();
        guard.dark = system_dark;
    };

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
    window: tauri::Window,
    state: State<'_, WindowState>,
) -> Result<()> {
    if window.label() != "main" {
        return Ok(());
    }

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

#[tauri::command]
pub fn go_back(app: AppHandle) -> Result<()> {
    let window = app
        .get_webview_window("main")
        .ok_or(SetupError::MainWindowMissing)?;
    window
        .eval("window.history.back();")
        .map_err(|e| SetupError::WindowOp {
            op: "history_back".into(),
            message: e.to_string(),
        })?;
    Ok(())
}

#[tauri::command]
pub fn open_in_new_window(
    route: RouteId,
    state: State<'_, WindowState>,
    app: AppHandle,
) -> Result<()> {
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        let path: &'static str = route.path();
        let title = route.title();
        let label = format!(
            "secondary:{}",
            path.trim_start_matches('/').replace('/', "-")
        );
        log::info!(
            "Opening new window for route {:?} (path: {}, label: {})",
            route,
            path,
            label
        );

        if let Some(mut existing_window) = app.get_webview_window(&label) {
            log::info!(
                "Secondary window already exists for label `{}`; focusing",
                label
            );
            let dark = {
                let guard = state.lock();
                guard.dark
            };
            #[cfg(target_os = "macos")]
            {
                super::setup_mac_window::update_appearance(&mut existing_window, dark)
                    .map_err(SetupError::appearance)?;
            }
            existing_window.show().map_err(|e| SetupError::WindowOp {
                op: "show_secondary_existing".into(),
                message: e.to_string(),
            })?;
            existing_window
                .set_focus()
                .map_err(|e| SetupError::WindowOp {
                    op: "focus_secondary_existing".into(),
                    message: e.to_string(),
                })?;
            return Ok(());
        }

        let main_window_size = app
            .get_webview_window("main")
            .and_then(|w| {
                w.inner_size()
                    .ok()
                    .map(|s| (s.width as f64, s.height as f64))
            })
            .unwrap_or((800.0, 600.0));

        #[allow(unused_mut)]
        let mut window = windows::with_secondary_bootstrap(
            { 
                #[allow(unused_mut)]
                let mut builder = WebviewWindowBuilder::new(
                    &app,
                    label.as_str(),
                    WebviewUrl::App(format!("{path}?secondary=true").into()),
                )
                .title(title)
                .decorations(true)
                .resizable(true)
                .maximizable(false)
                .minimizable(false)
                .maximized(false)
                .inner_size(main_window_size.0, main_window_size.1);
                
                #[cfg(target_os = "macos")]
                {
                    builder = builder
                    .title_bar_style(tauri::TitleBarStyle::Transparent);
                }

                builder
            }
        )
        .build()?;

        let dark = {
            let guard = state.lock();
            guard.dark
        };
        #[cfg(target_os = "macos")]
        {
            super::setup_mac_window::update_appearance(&mut window, dark)
                .map_err(SetupError::appearance)?;
        }
    }

    Ok(())
}
