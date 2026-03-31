use std::time::{SystemTime, UNIX_EPOCH};

use audio_system::rt::ExcitementSource;

use common::error::{InstrumentError, Result};
use common::instrument::PlaybackQuality;
use common::instrument::commands::{ReflectBandControlPayload, ReflectKeyControlPayload, SpectrumPayload};
use common::instrument::events::{BAND_CONTROL_G_K, KEY_CONTROL_G_K};
use common::instrument::{
    events::{ExcitementSourcePayload, PlaybackStatePayload},
    Layout,
};
use common::NodeKey;
use tauri::{AppHandle, Emitter, State};

use crate::{health::HealthSetupState};
use super::state::InstrumentState;

#[tauri::command]
/// Creates instrument engine and starts streaming
pub fn instrument_playback_start(state: State<'_, InstrumentState>, app: AppHandle) -> Result<()> {
    log::debug!("instrument_playback_start called");

    match state.start_playback()? {
        true => {
            log::info!("Starting playback");
        }
        false => {
            log::warn!("Playback already active; no action taken");
        }
    }

    app.emit(
        common::instrument::events::PLAYBACK_STATE,
        PlaybackStatePayload { playing: true },
    )
    .map_err(|e| InstrumentError::ResumeFailed {
        detail: Some(e.to_string()),
    })?;
    log::info!("Emitted playback state: playing");

    // Reflect will be emitted after the stream has started (InstrumentState.start_playback)

    Ok(())
}

#[tauri::command]
/// Stops stream and destroys instrument engine
pub fn instrument_playback_stop(state: State<'_, InstrumentState>, app: AppHandle) -> Result<()> {
    log::debug!("instrument_playback_stop called");

    match state.stop_playback()? {
        true => {
            log::info!("Stopping playback");
            app.emit(
                common::instrument::events::PLAYBACK_STATE,
                PlaybackStatePayload { playing: false },
            )
            .map_err(|e| InstrumentError::ResumeFailed {
                detail: Some(e.to_string()),
            })?;
            log::info!("Emitted playback state: stopped");
        }
        false => {
            log::warn!("Playback already stopped; no action taken");
        }
    }

    Ok(())
}

#[tauri::command]
/// Returns true if playback is active
pub fn instrument_playback_state(
    state: State<'_, InstrumentState>,
) -> Result<PlaybackStatePayload> {
    log::debug!("instrument_playback_state called");

    let playing = state.playing();

    log::debug!("Returning playback state: {}", playing);
    Ok(PlaybackStatePayload { playing })
}

#[tauri::command]
/// Returns current instrument excitement source (mic or entropy)
pub fn instrument_excitement_source(
    state: State<'_, InstrumentState>,
) -> Result<ExcitementSourcePayload> {
    log::debug!("instrument_excitement_source called");
    let source = state.excitement_source();
    log::debug!("Current excitement source (enum): {:?}", source);
    Ok(ExcitementSourcePayload {
        source: source.into(),
    })
}

#[tauri::command]
/// Returns current instrument excitement source (mic or entropy)
pub fn instrument_set_excitement_source(
    source: u8,
    state: State<'_, InstrumentState>,
    health: State<'_, HealthSetupState>,
    app: AppHandle,
) -> Result<()> {
    log::trace!(
        "instrument_set_excitement_source called with source={}",
        source
    );

    let src_u8 = source;
    let requested: ExcitementSource = source.into();
    let current: ExcitementSource = state.excitement_source();
    log::trace!(
        "Excitement source change requested: current={:?}, requested={:?} (code={})",
        current,
        requested,
        src_u8
    );

    // If no change, just log and return
    if current == requested {
        log::trace!(
            "Excitement source unchanged (still {:?}, code={}) - no action taken",
            current,
            src_u8
        );
        return Ok(());
    }

    // Read mic permission with a narrow lock scope
    let initial_mic_permission = {
        log::trace!("Locking HealthSetupState to read initial mic permission");
        let hs = health.lock();
        let perm = hs.initial_mic_permission;
        log::trace!("HealthSetupState.initial_mic_permission={:?}", perm);
        perm
    };

    // Permission check if Mic requested
    if matches!(requested, ExcitementSource::Mic) && initial_mic_permission != Some(true) {
        log::warn!("Won't enable mic excitement source without mic permission");
        return Err(InstrumentError::MicPermissionMissing.into());
    }

    // Apply change via inner method
    log::trace!("Invoking engine.set_excitement_source({:?})", requested);
    match state.set_excitement_source(requested)? {
        true => {
            log::info!(
                "Setting excitement source to {:?} (code={})",
                requested,
                src_u8
            );
            app.emit(
                common::instrument::events::EXCITEMENT_SRC,
                ExcitementSourcePayload {
                    source: requested.into(),
                },
            )
            .map_err(|e| InstrumentError::Emit {
                event: common::instrument::events::EXCITEMENT_SRC.to_string(),
                message: e.to_string(),
            })?;
            log::info!("Emitted excitement source event: code={}", src_u8);
        }
        false => {
            log::warn!(
                "Inner reported excitement source not changed for {:?} (code={})",
                requested,
                src_u8
            );
        }
    }

    Ok(())
}

#[tauri::command]
/// Pauses playback, maintaining state
pub fn instrument_playback_pause(state: State<'_, InstrumentState>, app: AppHandle) -> Result<()> {
    log::debug!("instrument_playback_pause called");

    match state.pause_playback()? {
        true => {
            log::info!("Pausing playback");
            app.emit(
                common::instrument::events::PLAYBACK_STATE,
                PlaybackStatePayload { playing: false },
            )
            .map_err(|e| InstrumentError::PauseFailed {
                detail: Some(e.to_string()),
            })?;
            log::info!("Emitted playback state: playing=false");
        }
        false => {
            log::warn!("Playback already paused; no action taken");
        }
    }

    Ok(())
}

#[tauri::command]
/// Resumes playback from paused state
pub fn instrument_playback_resume(
    state: State<'_, InstrumentState>,
    app: AppHandle,
) -> Result<()> {
    log::debug!("instrument_playback_resume called");

    match state.resume_playback()? {
        true => {
            log::info!("Resuming playback");
            app.emit(
                common::instrument::events::PLAYBACK_STATE,
                PlaybackStatePayload { playing: true },
            )
            .map_err(|e| InstrumentError::ResumeFailed {
                detail: Some(e.to_string()),
            })?;
            log::info!("Emitted playback state: playing=true");
        }
        false => {
            log::warn!("Playback already running; no action taken");
        }
    }

    Ok(())
}

#[tauri::command]
/// Returns current instrument layout (invoke/event: instrument_layout)
pub fn instrument_layout(state: State<'_, InstrumentState>) -> Result<Layout> {
    log::debug!("instrument_layout called");
    Ok(state.layout())
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
/// payload: `SafeArea`
pub fn ui_safe_area_insets_apply(
    top: f64,
    right: f64,
    bottom: f64,
    left: f64,
    state: State<'_, InstrumentState>,
    window_state: State<'_, crate::setup::WindowState>,
    app: AppHandle,
    window: tauri::Window,
) -> Result<()> {
    if window.label() != "main" {
        return Ok(());
    }
    log::debug!(
        "ui_safe_area_insets_apply called (UI override): top={}, right={}, bottom={}, left={}",
        top,
        right,
        bottom,
        left
    );

    let (top, right, bottom, left) = {
        // Persist UI safe area contribution in window state (non-additive override)
        let mut win = window_state.lock();
        win.ui_safe_area = common::safe_area::SafeArea {
            top,
            right,
            bottom,
            left,
        };

        (
            win.system_safe_area.top + top,
            win.system_safe_area.right + right,
            win.system_safe_area.bottom + bottom,
            win.system_safe_area.left + left,
        )
    };

    // Get current layout before changes
    let old_layout = state.layout();

    state
        .set_safe_area(top, right, bottom, left)
        .map_err(|e| InstrumentError::Emit {
            event: "set_safe_area".to_string(),
            message: e.to_string(),
        })?;

    let new_layout = state.layout();

    // Only emit LAYOUT event if layout actually changed
    if old_layout != new_layout {
        log::info!(
            "Applied UI safe area override [top: {}, right: {}, bottom: {}, left: {}] - layout changed",
            top,
            right,
            bottom,
            left
        );

        app.emit(common::instrument::events::LAYOUT, new_layout)
            .map_err(|e| InstrumentError::Emit {
                event: common::instrument::events::LAYOUT.to_string(),
                message: e.to_string(),
            })?;

        for node_key in new_layout.registry().all_keys() {
            let band = state.get_band_control(node_key)?;
            let key = state.get_key_control(node_key)?;
            app.emit(
                BAND_CONTROL_G_K,
                ReflectBandControlPayload { group: node_key.group(), key: node_key.key(), value: band },
            )?;
            app.emit(
                KEY_CONTROL_G_K,
                ReflectKeyControlPayload { group: node_key.group(), key: node_key.key(), value: key },
            )?;
        }

        let preset = state.get_preset();

        super::save_preset(preset, &app)?;
    } else {
        log::debug!(
            "UI safe area values unchanged [top: {}, right: {}, bottom: {}, left: {}] - skipping layout emission",
            top,
            right,
            bottom,
            left
        );
    }

    Ok(())
}

#[tauri::command]
pub fn instrument_string_snoop_data(
    group: usize,
    key: usize,
    state: tauri::State<'_, InstrumentState>,
) -> common::error::Result<common::instrument::data::StringSnoopDataResponse> {
    let samples = state.snapshot_output_snoop(NodeKey::new(group as u8, key as u8));
    Ok(common::instrument::data::StringSnoopDataResponse { samples })
}

#[tauri::command]
pub fn instrument_all_string_snoops(
    state: tauri::State<'_, InstrumentState>,
) -> common::error::Result<common::instrument::data::StringSnoopBatchPayload> {

    let entries = state
        .snapshot_all_output_snoops()
        .into_iter()
        .map(
            |(NodeKey(group, key), samples)| common::instrument::data::StringSnoopEntry {
                group,
                key,
                samples,
            },
        )
        .collect::<Vec<_>>();

    let t_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis_f64())
        .unwrap();

    Ok(common::instrument::data::StringSnoopBatchPayload {
        t_unix_ms,
        snoops: entries,
    })
}

#[tauri::command]
pub fn instrument_excitement_snoop_data(
    group: usize,
    key: usize,
    state: tauri::State<'_, InstrumentState>,
) -> common::error::Result<common::instrument::data::ExcitementSnoopDataResponse> {
    let samples = state.snapshot_excitement_snoop(NodeKey::new(group as u8, key as u8));
    Ok(common::instrument::data::ExcitementSnoopDataResponse { samples })
}

#[tauri::command]
pub fn instrument_all_excitement_snoops(
    state: tauri::State<'_, InstrumentState>,
) -> common::error::Result<common::instrument::data::ExcitementSnoopBatchPayload> {

    let entries = state
        .snapshot_all_excitement_snoops()
        .into_iter()
        .map(
            |(NodeKey(group, key), samples)| common::instrument::data::ExcitementSnoopEntry {
                group,
                key,
                samples,
            },
        )
        .collect::<Vec<_>>();

    let t_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis_f64())
        .unwrap();

    Ok(common::instrument::data::ExcitementSnoopBatchPayload {
        t_unix_ms,
        snoops: entries,
    })
}

#[tauri::command]
/// Updates the band control value for a specific key
pub fn instrument_update_band_control(
    keys: Vec<NodeKey>,
    increment: f32,
    state: State<'_, InstrumentState>,
    app: AppHandle,
) -> Result<()> {
    for node_key in keys {
        let value = state.get_band_control(node_key)?;
        let mut next_value = (value + increment).clamp(-1.0, 1.0);

        if value.signum() != next_value.signum() {
            next_value = 0.0 * next_value.signum()
        }

        state.set_band_control(node_key, next_value)?;

        log::trace!(
            "Band control updated for node ({node_key:?}): value={next_value}",
        );

        app.emit(
            BAND_CONTROL_G_K,
            ReflectBandControlPayload { group: node_key.group(), key: node_key.key(), value: next_value },
        )?;
    }

    let preset = state.get_preset();

    super::save_preset(preset, &app)?;

    Ok(())
}

/// Update key control state (pressed/released) for a specific key
#[tauri::command]
pub fn instrument_update_key_control(
    keys: Vec<NodeKey>,
    value: f32,
    state: State<'_, InstrumentState>,
    app: AppHandle,
) -> Result<()> {
    for node_key in keys {
        state.set_key_control(node_key, value)?;

        app.emit(
            KEY_CONTROL_G_K,
            ReflectKeyControlPayload { group: node_key.group(), key: node_key.key(), value },
        )?;


        log::trace!(
            "Key control updated for node ({node_key:?}): value={value} ({})",
            if value > 0.5 { "pressed" } else { "released" }
        );
    }

    let preset = state.get_preset();

    super::save_preset(preset, &app)?;

    Ok(())
}

#[tauri::command]
pub fn instrument_quality_indicator(state: State<'_, InstrumentState>) -> PlaybackQuality {
    state.quality_indicator()
}

#[tauri::command]
pub fn snapshot_processed_output_spectrum(state: State<'_, InstrumentState>) -> Option<SpectrumPayload> {
    let data = state.snapshot_processed_output_spectrum();

    data.map(|data| SpectrumPayload {
        data,
        t_unix_ms: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis_f64() ).unwrap()
    })
}

#[cfg(feature="devtools")]
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn instrument_edit_finetuned_values(
    siren_alpha: f32,
    group_ls_gain_db: f32,
    group_q: f32,
    node_follow_response_time_s: f32,
    filter_morph_follow_s: f32,
    filter_q_piercing: f32,
    filter_q_bright: f32,
    filter_q_shelf: f32,
    filter_shelf_gain_db: f32,
    filter_q_warm: f32,
    node_bell_q: f32,
    node_bell_gain_db: f32,
    formant_base_q: f32,
    state: State<'_, InstrumentState>,
) -> Result<common::commands::edit::FineTunedValuesPayload> {
    let values = common::commands::edit::FineTunedValuesPayload{
        siren_alpha,
        group_ls_gain_db,
        group_q,
        filter_morph_follow_s,
        node_follow_response_time_s,
        filter_q_piercing,
        filter_q_bright,
        filter_q_shelf,
        filter_shelf_gain_db,
        filter_q_warm,
        node_bell_q,
        node_bell_gain_db,
        formant_base_q,
    };
    // Update the fine-tuned values
    state.set_finetuned_values(
        values
    )?;

    log::info!("Fine-tuned values updated via devtools: {values:#?}");

    state.get_finetuned_values()
}

#[cfg(feature="devtools")]
#[tauri::command]
pub async fn instrument_get_finetuned_values(
    state: State<'_, InstrumentState>,
) -> Result<common::commands::edit::FineTunedValuesPayload> {
    state.get_finetuned_values()
}
