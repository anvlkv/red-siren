use serde_json::Value;
use tauri::{App, Emitter, Listener, Manager};
use tauri::async_runtime::spawn;
use tauri_plugin_store::StoreExt;
use common::error::{Result};
use common::tuner::{Config, Layout as TunerLayout};

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

                // Initialize tuner config from instrument layout on first layout event if empty
                let should_init = { state.tuner_config.read().sensor_data.is_empty() };
                if should_init {
                    let new_config: Config = inst_layout.into();
                    {
                        let mut cfg = state.tuner_config.write();
                        *cfg = new_config.clone();
                    }
                    // Emit and persist new config so UI picks it up
                    handle.emit(common::events::tuner::CONFIG, new_config.clone()).ok();
                    if let Ok(store) = handle.store(TUNER_STORE_NAME) {
                        store.set(TUNER_CONFIG_KEY, serde_json::to_value(&new_config).unwrap_or(Value::Null));
                    }
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
