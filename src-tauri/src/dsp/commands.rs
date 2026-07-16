use std::sync::Arc;

use parking_lot::RwLock;
use tauri::{Emitter, State};

use crate::dsp::{DspError, ShaperCallbackArgs, SirenConfig};

use super::{Analyzer, Synthesizer};

#[derive(Default)]
pub struct DspState {
    pub synth: RwLock<Option<Arc<Synthesizer>>>,
    pub analyze: RwLock<Option<Arc<Analyzer>>>,
}

#[tauri::command]
pub async fn create_synth(
    dsp_state: State<'_, DspState>,
    app_handle: tauri::AppHandle,
) -> Result<(), DspError> {
    let mut synth_lock = dsp_state.synth.write();

    if synth_lock.is_none() {
        let synth = Synthesizer::new();

        *synth_lock = Some(Arc::new(synth));

        log::info!("Synthesizer created");
    } else {
        log::debug!("Synthesizer already exists");
    }

    let n_chambers = synth_lock.as_ref().unwrap().siren.chambers.len();
    let resolution = synth_lock.as_ref().unwrap().siren.resolution;

    app_handle
        .emit(
            "siren-info",
            SirenConfig {
                resolution,
                n_chambers: n_chambers as u32,
            },
        )
        .map_err(|e| {
            log::error!("{e}");
            DspError::TauriError("Failed to emit siren-info event".to_string())
        })?;

    Ok(())
}

#[tauri::command]
pub async fn request_siren_info(
    dsp_state: State<'_, DspState>,
    app_handle: tauri::AppHandle,
) -> Result<(), DspError> {
    let synth_lock = dsp_state.synth.read();

    let Some(synth) = synth_lock.as_ref() else {
        return Err(DspError::SynthNotInitialized);
    };

    let n_chambers = synth.siren.chambers.len();
    let resolution = synth.siren.resolution;

    app_handle
        .emit(
            "siren-info",
            SirenConfig {
                resolution,
                n_chambers: n_chambers as u32,
            },
        )
        .map_err(|e| {
            log::error!("{e}");
            DspError::TauriError("Failed to emit siren-info event".to_string())
        })?;

    Ok(())
}

#[tauri::command]
pub async fn request_chamber_info(
    chamber_index: usize,
    dsp_state: State<'_, DspState>,
    app_handle: tauri::AppHandle,
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

    let info = chamber.info();

    app_handle
        .emit(&format!("chamber-{chamber_index}-info"), info)
        .map_err(|e| {
            log::error!("{e}");
            DspError::TauriError("Failed to emit chamber info".to_string())
        })?;

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
    app_handle: tauri::AppHandle,
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

    let info = chamber.info();

    app_handle
        .emit(&format!("chamber-{chamber_index}-info"), info)
        .map_err(|e| {
            log::error!("{e}");
            DspError::TauriError("Failed to emit chamber info".to_string())
        })?;

    Ok(())
}

#[tauri::command]
pub async fn set_shape(
    chamber_index: usize,
    dsp_state: State<'_, DspState>,
    app_handle: tauri::AppHandle,
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

    let info = chamber.shape(
        |ShaperCallbackArgs {
             chunk_index,
             num_chunks,
             index,
             opening_width,
             gap_width,
             previous_chord,
         }: &ShaperCallbackArgs| 1.0,
    );

    app_handle
        .emit(&format!("chamber-{chamber_index}-info"), info)
        .map_err(|e| {
            log::error!("{e}");
            DspError::TauriError("Failed to emit chamber info".to_string())
        })?;

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
