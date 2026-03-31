use std::sync::OnceLock;

use tauri::{
    AppHandle, Manager, Runtime, TitleBarStyle, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

const MAIN_WINDOW_LABEL: &str = "main";
const SPLASH_WINDOW_LABEL: &str = "splashscreen";
const PRODUCT_NAME: &str = "Red Siren";
const DEFAULT_WIDTH: f64 = 800.0;
const DEFAULT_HEIGHT: f64 = 600.0;
const DEVTOOLS_ENABLED: bool = cfg!(feature = "devtools");

/// Ensure the primary (main + splash) windows exist. Safe to call multiple times.
pub fn ensure_startup_windows(app: &AppHandle) -> tauri::Result<()> {
    match app.get_webview_window(MAIN_WINDOW_LABEL) {
        Some(win) => ensure_bootstrap_flags(&win, false),
        None => create_main_window(app)?,
    }

    match app.get_webview_window(SPLASH_WINDOW_LABEL) {
        Some(win) => ensure_bootstrap_flags(&win, false),
        None => create_splash_window(app)?,
    }

    Ok(())
}

/// Attach bootstrap flags for windows that behave like the primary window.
pub fn with_primary_bootstrap<'a, R: Runtime, M: Manager<R>>(
    builder: WebviewWindowBuilder<'a, R, M>,
) -> WebviewWindowBuilder<'a, R, M> {
    builder.initialization_script(bootstrap_flags_script(false))
}

/// Attach bootstrap flags for secondary / pop-out windows.
pub fn with_secondary_bootstrap<'a, R: Runtime, M: Manager<R>>(
    builder: WebviewWindowBuilder<'a, R, M>,
) -> WebviewWindowBuilder<'a, R, M> {
    builder.initialization_script(bootstrap_flags_script(true))
}

fn create_main_window(app: &AppHandle) -> tauri::Result<()> {
    with_primary_bootstrap(
        WebviewWindowBuilder::new(app, MAIN_WINDOW_LABEL, WebviewUrl::App("/".into()))
            .title(PRODUCT_NAME)
            .decorations(true)
            .visible(false)
            .title_bar_style(TitleBarStyle::Transparent)
            .resizable(true)
            .inner_size(DEFAULT_WIDTH, DEFAULT_HEIGHT),
    )
    .build()
    .map(|_| ())
}

fn create_splash_window(app: &AppHandle) -> tauri::Result<()> {
    with_primary_bootstrap(
        WebviewWindowBuilder::new(
            app,
            SPLASH_WINDOW_LABEL,
            WebviewUrl::App("splashscreen.html".into()),
        )
        .title(PRODUCT_NAME)
        .decorations(false)
        .visible(true)
        .shadow(true),
    )
    .build()
    .map(|_| ())
}

fn ensure_bootstrap_flags(window: &WebviewWindow, is_secondary: bool) {
    if let Err(e) = window.eval(bootstrap_flags_script(is_secondary)) {
        log::warn!(
            "failed to inject bootstrap flags into `{}`: {}",
            window.label(),
            e
        );
    }
}

fn bootstrap_flags_script(is_secondary: bool) -> &'static str {
    if is_secondary {
        static SECONDARY: OnceLock<String> = OnceLock::new();
        SECONDARY.get_or_init(|| build_flags_script(true))
    } else {
        static PRIMARY: OnceLock<String> = OnceLock::new();
        PRIMARY.get_or_init(|| build_flags_script(false))
    }
}

fn build_flags_script(is_secondary: bool) -> String {
    let devtools = if DEVTOOLS_ENABLED { "true" } else { "false" };
    let secondary = if is_secondary { "true" } else { "false" };
    [
        "(function(){",
        "if(!Object.prototype.hasOwnProperty.call(window,'__RED_SIREN_FLAGS__')){",
        "const flags=Object.freeze({devtools:",
        devtools,
        ",secondary:",
        secondary,
        "});",
        "Object.defineProperty(window,'__RED_SIREN_FLAGS__',{value:flags,writable:false,configurable:false});",
        "}",
        "})();",
    ]
    .join("")
}
