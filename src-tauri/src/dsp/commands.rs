use std::sync::Arc;

use parking_lot::RwLock;
use tauri::State;

use crate::dsp::DspError;

use super::{Analyzer, Synthesizer};

#[derive(Default)]
pub struct DspState {
    pub synth: RwLock<Option<Arc<Synthesizer>>>,
    pub analyze: RwLock<Option<Arc<Analyzer>>>,
}

#[tauri::command]
pub async fn create_synth(dsp_state: State<'_, DspState>) -> Result<(), DspError> {
    let mut synth_lock = dsp_state.synth.write();

    if synth_lock.is_none() {
        let synth = Synthesizer::new();
        *synth_lock = Some(Arc::new(synth));

        log::info!("Synthesizer created");
    } else {
        log::debug!("Synthesizer already exists");
    }
    Ok(())
}
