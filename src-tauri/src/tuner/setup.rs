use serde_json::Value;
use std::collections::HashMap;
use tauri::{App, Emitter, Listener, Manager};
use tauri::async_runtime::spawn;
use tauri_plugin_store::StoreExt;
use common::error::{Result};
use common::tuner::{Config, Layout as TunerLayout};
use common::{NodeKey, NodeKeyRegistry};

use super::TunerState;

const TUNER_STORE_NAME: &str = "tuner.json";
const TUNER_CONFIG_KEY: &str = "config";

/// Setup tuner: manage state, restore persisted config, wire event listeners, persist on change.
pub fn setup(app: &mut App) -> Result<()> {
    let _is_new = app.manage(TunerState::new());

    // Restore persisted config if present
    if let Ok(store) = app.store(TUNER_STORE_NAME) {
        if let Some(saved) = store
            .get(TUNER_CONFIG_KEY)
            .and_then(|v: Value| serde_json::from_value::<Config>(v).ok())
        {
            // Update state with persisted config
            let state = app.state::<TunerState>();
            {
                let mut cfg = state.tuner_config.write();
                *cfg = saved.clone();
            }
            // Emit current config so UI picks it up
            app.emit(common::events::tuner::CONFIG, saved).ok();
        }
    }

    // Persist config upon change
    let handle = app.handle().clone();
    app.listen(common::events::tuner::CONFIG, move |_| {
        let handle = handle.clone();
        spawn(async move {
            let state = handle.state::<TunerState>();
            let cfg = state.tuner_config.read().clone();
            if let Ok(store) = handle.store(TUNER_STORE_NAME) {
                store.set(TUNER_CONFIG_KEY, serde_json::to_value(&cfg).unwrap_or(Value::Null));
            }
        });
    });

    // Listen for instrument layout changes, derive tuner layout, and emit.
    let handle = app.handle().clone();
    app.listen(common::instrument::events::LAYOUT, move |e| {
        let handle = handle.clone();
        let json = e.payload();
        match serde_json::from_str::<common::instrument::Layout>(json) {
            Ok(inst_layout) => {
                let tuner_layout: TunerLayout = inst_layout.into();

                let state = handle.state::<TunerState>();
                {
                    *state.last_instrument_layout.write() = Some(inst_layout);
                    *state.current_layout.write() = Some(tuner_layout);
                }

                // Always regenerate tuner config on layout changes to ensure proper synchronization
                let should_regenerate = {
                    let current_config = state.tuner_config.read();
                    let registry = NodeKeyRegistry::new(
                        inst_layout.num_groups.get(),
                        inst_layout.num_keys_per_group.get(),
                    );

                    // Check if current sensor data has invalid NodeKeys or is empty
                    let has_invalid_keys = !registry.has_invalid_keys(&current_config.sensor_data.iter()
                        .map(|s| (s.key, ()))
                        .collect()).is_empty();

                    let is_empty = current_config.sensor_data.is_empty();
                    let wrong_count = current_config.sensor_data.len() != registry.total_keys();

                    // Always regenerate to ensure synchronization, but log the reason
                    if is_empty {
                        log::info!("Regenerating tuner config: empty sensor data");
                    } else if has_invalid_keys {
                        log::info!("Regenerating tuner config: invalid NodeKeys detected");
                    } else if wrong_count {
                        log::info!("Regenerating tuner config: sensor count mismatch (expected: {}, got: {})",
                                  registry.total_keys(), current_config.sensor_data.len());
                    } else {
                        log::info!("Regenerating tuner config: ensuring synchronization with layout change");
                    }

                    true // Always regenerate to ensure synchronization
                };

                if should_regenerate {
                    log::info!("Regenerating tuner config for layout: groups={}, keys_per_group={}",
                        inst_layout.num_groups.get(), inst_layout.num_keys_per_group.get());

                    // Preserve old sensor frequency settings if possible
                    let old_sensor_settings = {
                        let current_config = state.tuner_config.read();
                        current_config.sensor_data.iter()
                            .map(|s| (s.key, (s.min_frequency, s.max_frequency, s.min_magnitude, s.max_magnitude)))
                            .collect::<HashMap<NodeKey, (f32, f32, f32, f32)>>()
                    };

                    let mut new_config: Config = inst_layout.into();

                    // Restore preserved settings where possible
                    for sensor in &mut new_config.sensor_data {
                        if let Some((min_freq, max_freq, min_mag, max_mag)) = old_sensor_settings.get(&sensor.key) {
                            sensor.min_frequency = *min_freq;
                            sensor.max_frequency = *max_freq;
                            sensor.min_magnitude = *min_mag;
                            sensor.max_magnitude = *max_mag;
                            log::debug!("Preserved sensor settings for key: {:?}", sensor.key);
                        }
                    }

                    {
                        let mut cfg = state.tuner_config.write();
                        *cfg = new_config.clone();
                    }

                    // Emit and persist new config so UI picks it up
                    handle.emit(common::events::tuner::CONFIG, new_config.clone()).ok();
                    if let Ok(store) = handle.store(TUNER_STORE_NAME) {
                        store.set(TUNER_CONFIG_KEY, serde_json::to_value(&new_config).unwrap_or(Value::Null));
                    }

                    log::info!("Tuner config regenerated with {} sensors", new_config.sensor_data.len());
                }

                if let Err(err) = handle.emit(common::events::tuner::LAYOUT, tuner_layout) {
                    log::error!("Failed emitting tuner layout: {}", err);
                }
            }
            Err(err) => {
                log::error!("Failed parsing instrument layout payload for tuner mapping: {}", err);
            }
        }
    });

    Ok(())
}
