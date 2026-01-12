use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use std::thread;
use tauri::{App, AppHandle, Emitter, Listener, Manager};

use common::{
    error::Result,
    events::health::SETUP_STATE,
    events::setup::{UPDATE_WINDOW_APPEARANCE, UPDATE_WINDOW_SIZE},
    instrument::{
        commands::{ReflectBandControlPayload, ReflectKeyControlPayload},
        events::{BAND_CONTROL_G_K, KEY_CONTROL_G_K, LAYOUT},
        Preset,
    },
};

use crate::instrument::{save_preset, InstrumentState};
use crate::setup::WindowState;

/// Central coordinator for cross-module app lifecycle and window-related events.
///
/// Why:
/// - Reduce hidden coupling spread across modules by centralizing who reacts to global events.
/// - Keep heavy operations (layout recompute, per-key reflect emits, persistence) off listeners with async tasks.
/// - Debounce preset saving to avoid excessive IO writes during resize/appearance changes.
///
/// Scope (Phase 1):
/// - Handle GUI-ready, window appearance, and window size updates.
/// - Recompute instrument layout, emit layout and reflect events, and debounce preset persistence.
pub struct AppBus {
    app: AppHandle,
    // Debounce guard for preset saves to reduce write frequency.
    presets_save_guard: Arc<Mutex<Debounce>>,
}

struct Debounce {
    last: Option<Instant>,
    min_interval: Duration,
}

impl Debounce {
    fn new(min_interval: Duration) -> Self {
        Self {
            last: None,
            min_interval,
        }
    }

    fn should_run(&mut self) -> bool {
        let now = Instant::now();
        match self.last {
            None => {
                self.last = Some(now);
                true
            }
            Some(prev) => {
                if now.duration_since(prev) >= self.min_interval {
                    self.last = Some(now);
                    true
                } else {
                    false
                }
            }
        }
    }
}

impl AppBus {
    pub fn setup(app: &mut App) -> Result<()> {
        let bus = Self {
            app: app.handle().clone(),
            presets_save_guard: Arc::new(Mutex::new(Debounce::new(Duration::from_millis(250)))),
        };

        bus.subscribe_gui_ready()?;
        bus.subscribe_window_appearance()?;
        bus.subscribe_window_size()?;

        Ok(())
    }

    fn subscribe_gui_ready(&self) -> Result<()> {
        let handle = self.app.clone();
        self.app.listen(SETUP_STATE, move |event| {
            let handle = handle.clone();
            // Parse payload to detect GUI readiness
            let payload = event.payload();
            // Best-effort parse; on failure, just ignore.
            #[derive(serde::Deserialize)]
            struct SetupStatePayload {
                gui_ready: bool,
                #[allow(dead_code)]
                mic_permission: Option<bool>,
                #[allow(dead_code)]
                devtools: bool,
            }
            if let Ok(ss) = serde_json::from_str::<SetupStatePayload>(payload) {
                if ss.gui_ready {
                    thread::spawn(move || {
                        let state = handle.state::<InstrumentState>();
                        let layout = state.layout();
                        if let Err(e) = handle.emit(LAYOUT, layout) {
                            log::error!(
                                "AppBus: failed emitting instrument layout on GUI ready: {e}"
                            );
                        }

                        // Emit reflect from persisted preset so UI aligns before playback starts
                        let preset = state.get_preset();
                        for node_key in layout.registry().all_keys() {
                            let band_value = preset.get_band_value(&node_key).unwrap_or(0.0);
                            let key_value = preset.get_key_value(&node_key).unwrap_or(0.0);

                            if let Err(e) = handle.emit(
                                BAND_CONTROL_G_K,
                                ReflectBandControlPayload {
                                    group: node_key.group(),
                                    key: node_key.key(),
                                    value: band_value,
                                },
                            ) {
                                log::error!(
                                    "AppBus: Failed emitting band reflect on GUI ready for {:?}: {e}",
                                    node_key
                                );
                            }

                            if let Err(e) = handle.emit(
                                KEY_CONTROL_G_K,
                                ReflectKeyControlPayload {
                                    group: node_key.group(),
                                    key: node_key.key(),
                                    value: key_value,
                                },
                            ) {
                                log::error!(
                                    "AppBus: Failed emitting key reflect on GUI ready for {:?}: {e}",
                                    node_key
                                );
                            }
                        }
                    });
                }
            }
        });
        Ok(())
    }

    fn subscribe_window_appearance(&self) -> Result<()> {
        let handle = self.app.clone();
        let presets_guard = Arc::clone(&self.presets_save_guard);
        self.app.listen(UPDATE_WINDOW_APPEARANCE, move |_| {
            let handle = handle.clone();
            let presets_guard = Arc::clone(&presets_guard);
            thread::spawn(move || {
                let instrument = handle.state::<InstrumentState>();
                let win_state = handle.state::<WindowState>();
                let is_dark = win_state.lock().dark;

                if let Err(e) = instrument.set_is_dark(is_dark) {
                    log::error!(
                        "AppBus: error updating `{}` dark mode: {e}",
                        UPDATE_WINDOW_APPEARANCE
                    );
                }

                let layout = instrument.layout();
                if let Err(e) = handle.emit(LAYOUT, layout) {
                    log::error!("AppBus: failed emitting instrument layout (appearance): {e}");
                }

                // Emit reflect events safely
                emit_reflect_events(&handle, &instrument);

                // Debounced preset save
                maybe_debounced_save_preset(&handle, &instrument, &presets_guard);
            });
        });
        Ok(())
    }

    fn subscribe_window_size(&self) -> Result<()> {
        let handle = self.app.clone();
        let presets_guard = Arc::clone(&self.presets_save_guard);
        self.app.listen(UPDATE_WINDOW_SIZE, move |_| {
            let handle = handle.clone();
            let presets_guard = Arc::clone(&presets_guard);
            thread::spawn(move || {
                let instrument = handle.state::<InstrumentState>();
                let win_state = handle.state::<WindowState>();
                let window_state = win_state.lock();
                let width = window_state.width;
                let height = window_state.height;

                match instrument.set_size(width, height) {
                    Ok(_) => {
                        let layout = instrument.layout();
                        if let Err(e) = handle.emit(LAYOUT, layout) {
                            log::error!("AppBus: failed emitting instrument layout (size): {e}");
                        }
                        // Emit reflect events safely
                        emit_reflect_events(&handle, &instrument);

                        // Debounced preset save
                        maybe_debounced_save_preset(&handle, &instrument, &presets_guard);
                    }
                    Err(e) => {
                        log::error!(
                            "AppBus: error updating `{}` size {}x{}: {e}",
                            UPDATE_WINDOW_SIZE,
                            width,
                            height
                        );
                    }
                }
            });
        });
        Ok(())
    }
}

/// Emit Reflect events for band and key controls per node.
///
/// Why:
/// - UI needs per-key updated control values after layout changes (size/appearance).
/// - Keep emit errors contained; do not panic on failures.
fn emit_reflect_events(handle: &AppHandle, instrument: &InstrumentState) {
    let new_layout = instrument.layout();
    for node_key in new_layout.registry().all_keys() {
        let band_value = match instrument.get_band_control(node_key) {
            Ok(v) => v,
            Err(e) => {
                log::error!("AppBus: get_band_control failed for {:?}: {e}", node_key);
                continue;
            }
        };
        let key_value = match instrument.get_key_control(node_key) {
            Ok(v) => v,
            Err(e) => {
                log::error!("AppBus: get_key_control failed for {:?}: {e}", node_key);
                continue;
            }
        };

        if let Err(e) = handle.emit(
            BAND_CONTROL_G_K,
            ReflectBandControlPayload {
                group: node_key.group(),
                key: node_key.key(),
                value: band_value,
            },
        ) {
            log::error!(
                "AppBus: Failed emitting band reflect for {:?}: {e}",
                node_key
            );
        }

        if let Err(e) = handle.emit(
            KEY_CONTROL_G_K,
            ReflectKeyControlPayload {
                group: node_key.group(),
                key: node_key.key(),
                value: key_value,
            },
        ) {
            log::error!(
                "AppBus: Failed emitting key reflect for {:?}: {e}",
                node_key
            );
        }
    }
}

/// Debounced preset persistence.
///
/// Why:
/// - Window events can fire rapidly; saving on every tick causes IO churn.
/// - Debounce to a minimal interval to avoid excessive writes while keeping state reasonably fresh.
fn maybe_debounced_save_preset(
    handle: &AppHandle,
    instrument: &InstrumentState,
    guard: &Arc<Mutex<Debounce>>,
) {
    let mut g = guard.lock();
    if g.should_run() {
        let preset: Preset = instrument.get_preset();
        if let Err(e) = save_preset(preset, handle) {
            log::error!("AppBus: error saving presets: {e}");
        } else {
            log::debug!("AppBus: presets saved (debounced)");
        }
    } else {
        log::trace!("AppBus: presets save skipped by debounce");
    }
}
