use crate::instrument::InstrumentState;
use crate::persistence::persistence::{load_json_or_default, save_json};
use common::error::Result;
use common::tuner::{Config, Layout as TunerLayout};
use std::thread;
use tauri::{App, AppHandle, Emitter, Listener, Manager};

use super::TunerState;

const TUNER_STORE_NAME: &str = "tuner.json";
const TUNER_CONFIG_KEY: &str = "config";

pub fn save_tuner_config(app: &AppHandle, config: Config) -> Result<()> {
    save_json(app, TUNER_STORE_NAME, TUNER_CONFIG_KEY, &config)
}

/// Setup tuner: manage state, restore persisted config, wire event listeners, persist on change.
pub fn setup(app: &mut App) -> Result<()> {
    // Restore persisted config if present, then rebase it on the current instrument layout.
    let persisted_config: Config =
        load_json_or_default(app.handle(), TUNER_STORE_NAME, TUNER_CONFIG_KEY)?;
    let config = if let Some(instrument) = app.try_state::<InstrumentState>() {
        let instrument_layout = instrument.layout();
        let registry = instrument_layout.registry();
        let tuner_layout: TunerLayout = instrument_layout.into();

        Config::new_from_previous(
            tuner_layout,
            instrument.sample_rate() as f32,
            audio_system::FFT_WINDOW_SIZE,
            registry,
            &persisted_config,
        )
    } else {
        log::warn!(
            "Instrument state unavailable during tuner setup; using persisted tuner config as-is"
        );
        persisted_config.clone()
    };

    if config != persisted_config {
        save_tuner_config(app.handle(), config.clone())?;
    }

    // Emit current config so UI picks it up
    app.emit(common::events::tuner::CONFIG, config.clone())?;

    let _is_new = app.manage(TunerState::new(app.handle().clone(), config));

    // Persist config upon change
    let handle = app.handle().clone();
    app.listen(common::events::tuner::CONFIG, move |_| {
        let handle = handle.clone();
        thread::spawn(move || {
            let state = handle.state::<TunerState>();
            let cfg = state.tuner_config();
            if let Err(e) = save_json(&handle, TUNER_STORE_NAME, TUNER_CONFIG_KEY, &cfg) {
                log::error!("Failed saving tuner config: {e}");
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
                        if let Err(e) =
                            save_json(&handle, TUNER_STORE_NAME, TUNER_CONFIG_KEY, &new_config)
                        {
                            log::error!("Failed saving tuner config after layout update: {e}");
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        log::error!("Error updating tuner layout: {e}")
                    }
                }

                if let Err(err) = handle.emit(common::events::tuner::LAYOUT, tuner_layout) {
                    log::error!("Failed emitting tuner layout: {}", err);
                }
            }
            Err(err) => {
                log::error!(
                    "Failed parsing instrument layout payload for tuner mapping: {}",
                    err
                );
            }
        }
    });

    Ok(())
}
