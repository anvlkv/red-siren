//! Audio Worklet Integration Module
//!
//! PURPOSE
//! -------
//! Main module for Web Audio API + AudioWorklet integration. Provides the
//! WebAudioController implementation and manages the hidden webview that
//! runs the AudioWorklet processor.
//!
//! ARCHITECTURE
//! ------------
//! This module bridges the Tauri backend with Web Audio API:
//! 1. WebAudioController implements StreamController trait
//! 2. Hidden webview loads audio-worklet.html and WASM module
//! 3. Commands handle responses from the hidden webview
//! 4. Shared state coordinates async operations
//!
//! MAYA DRY KISS
//! -------------
//! - Clean module organization with focused responsibilities
//! - Simple webview creation and lifecycle management
//! - Clear separation between controller, state, and commands

pub mod commands;
pub mod controller;
pub mod state;

pub use controller::WebAudioController;
pub use state::WorkletState;

use std::sync::Arc;
use tauri::{App, AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// Create the hidden webview for running AudioWorklet
pub fn create_hidden_webview(app: &AppHandle) -> tauri::Result<()> {
    log::info!("Creating hidden webview for audio worklet");

    let webview = WebviewWindowBuilder::new(app, "audio-worklet", WebviewUrl::App("audio-worklet.html".into()))
        .title("Audio Worklet (Hidden)")
        .visible(false) // Hidden webview
        .resizable(false)
        .minimizable(false)
        .maximizable(false)
        .closable(false)
        .skip_taskbar(true)
        .inner_size(400.0, 300.0) // Small size since it's hidden
        .build()?;

    log::info!("Hidden webview created successfully: {}", webview.label());
    Ok(())
}

/// Initialize audio worklet state and create hidden webview during app setup
pub fn setup_audio_worklet(app: &mut App) -> tauri::Result<()> {
    log::info!("Setting up audio worklet integration");

    // Create and manage WorkletState
    let worklet_state = WorkletState::new();
    app.manage(worklet_state);

    // Create hidden webview after app is ready
    let app_handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        // Wait a bit for the main window to be ready
        tauri::async_runtime::spawn_blocking(move || {
            std::thread::sleep(std::time::Duration::from_millis(1000));

            if let Err(e) = create_hidden_webview(&app_handle) {
                log::error!("Failed to create hidden webview: {}", e);
            }
        });
    });

    log::info!("Audio worklet setup completed");
    Ok(())
}

/// Create a WebAudioController instance
pub fn create_web_audio_controller(app: &AppHandle) -> WebAudioController {
    let state = app.state::<Arc<WorkletState>>();
    WebAudioController::new(app.clone(), state.inner().clone())
}
