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

    // Restore persisted config if present
    let config = app.store(TUNER_STORE_NAME).ok().and_then(|store| {
        store
            .get(TUNER_CONFIG_KEY)
            .and_then(|v: Value| serde_json::from_value::<Config>(v).ok())
    }).unwrap_or_default();

    // Emit current config so UI picks it up
    app.emit(common::events::tuner::CONFIG, config.clone())?;

    let _is_new = app.manage(TunerState::new(app.handle().clone(), config));


    // Persist config upon change
    let handle = app.handle().clone();
    app.listen(common::events::tuner::CONFIG, move |_| {
        let handle = handle.clone();
        spawn(async move {
            let state = handle.state::<TunerState>();
            let cfg = state.tuner_config();
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
                let registry = inst_layout.registry();

                match state.update_layout(tuner_layout, registry) {
                    Ok(Some(new_config)) => {
                        if let Ok(store) = handle.store(TUNER_STORE_NAME) {
                            store.set(TUNER_CONFIG_KEY, serde_json::to_value(&new_config).unwrap_or(Value::Null));
                        }
                    }
                    Ok(None) => {},
                    Err(e) => {
                        log::error!("Error updating tuner layout: {e}")
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
