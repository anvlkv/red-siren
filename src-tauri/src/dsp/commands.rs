use std::sync::Arc;

use parking_lot::RwLock;
use tauri::State;

use crate::dsp::{DspError, ShaperCallbackArgs};

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

#[tauri::command]
pub async fn set_speed(
    speed: u32,
    chamber_index: usize,
    dsp_state: State<'_, DspState>,
) -> Result<(), DspError> {
    let synth_lock = dsp_state.synth.read();

    let Some(synth) = synth_lock.as_ref() else {
        return Err(DspError::SynthNotInitialized);
    };

    let Some(chamber) = synth.siren.chambers.get(chamber_index) else {
        return Err(DspError::NoChamberWithIndex(
            chamber_index,
            synth.siren.chambers.len(),
        ));
    };

    chamber
        .speed
        .store(speed, std::sync::atomic::Ordering::Relaxed);

    Ok(())
}

#[tauri::command]
pub async fn set_window(
    window: u32,
    chamber_index: usize,
    dsp_state: State<'_, DspState>,
) -> Result<(), DspError> {
    let synth_lock = dsp_state.synth.read();

    let Some(synth) = synth_lock.as_ref() else {
        return Err(DspError::SynthNotInitialized);
    };

    let Some(chamber) = synth.siren.chambers.get(chamber_index) else {
        return Err(DspError::NoChamberWithIndex(
            chamber_index,
            synth.siren.chambers.len(),
        ));
    };

    chamber
        .window_size
        .store(window, std::sync::atomic::Ordering::Relaxed);

    Ok(())
}

#[tauri::command]
pub async fn set_shape(
    chamber_index: usize,
    dsp_state: State<'_, DspState>,
) -> Result<(), DspError> {
    let synth_lock = dsp_state.synth.read();

    let Some(synth) = synth_lock.as_ref() else {
        return Err(DspError::SynthNotInitialized);
    };

    let Some(chamber) = synth.siren.chambers.get(chamber_index) else {
        return Err(DspError::NoChamberWithIndex(
            chamber_index,
            synth.siren.chambers.len(),
        ));
    };

    chamber.shape(
        |ShaperCallbackArgs {
             chunk_index,
             num_chunks,
             index,
             opening_width,
             gap_width,
         }: &ShaperCallbackArgs| 1.0,
    );

    Ok(())
}

#[tauri::command]
pub async fn get_snapshot(dsp_state: State<'_, DspState>) -> Result<Vec<[f32; 2]>, DspError> {
    let synth_lock = dsp_state.synth.read();

    let Some(synth) = synth_lock.as_ref() else {
        return Err(DspError::SynthNotInitialized);
    };

    Ok(synth.siren_snapshot.get_snapshot())
}
