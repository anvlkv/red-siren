use audio_system::rt::ExcitementSource;

use common::error::{InstrumentError, Result};
use common::instrument::commands::{UpdateBandControlPayload, UpdateKeyControlPayload};
use common::instrument::events::{BAND_CONTROL_G_K, KEY_CONTROL_G_K};
use common::instrument::{
    events::{ExcitementSourcePayload, PlaybackStatePayload},
    Layout,
};
use common::NodeKey;
use tauri::{AppHandle, Emitter, State};

use crate::{health::HealthSetupState, instrument::engine::InstrumentEngine};

#[tauri::command]
/// Creates instrument engine and starts streaming
pub fn instrument_playback_start(state: State<'_, InstrumentEngine>, app: AppHandle) -> Result<()> {
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

    Ok(())
}

#[tauri::command]
/// Stops stream and destroys instrument engine
pub fn instrument_playback_stop(state: State<'_, InstrumentEngine>, app: AppHandle) -> Result<()> {
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
    state: State<'_, InstrumentEngine>,
) -> Result<PlaybackStatePayload> {
    log::debug!("instrument_playback_state called");

    let playing = state.playing();

    log::debug!("Returning playback state: {}", playing);
    Ok(PlaybackStatePayload { playing })
}

#[tauri::command]
/// Returns current instrument excitement source (mic or entropy)
pub fn instrument_excitement_source(
    state: State<'_, InstrumentEngine>,
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
    state: State<'_, InstrumentEngine>,
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
    let mic_permission_opt = {
        log::trace!("Locking HealthSetupState to read mic_permission");
        let hs = health.lock();
        let perm = hs.mic_permission;
        log::trace!("HealthSetupState.mic_permission={:?}", perm);
        perm
    };

    // Permission check if Mic requested
    if matches!(requested, ExcitementSource::Mic) && mic_permission_opt != Some(true) {
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
pub fn instrument_playback_pause(state: State<'_, InstrumentEngine>, app: AppHandle) -> Result<()> {
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
    state: State<'_, InstrumentEngine>,
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
pub fn instrument_layout(state: State<'_, InstrumentEngine>) -> Result<Layout> {
    log::debug!("instrument_layout called");
    Ok(state.layout())
}

#[tauri::command]
/// payload: `SafeArea`
pub fn ui_safe_area_insets_apply(
    top: f64,
    right: f64,
    bottom: f64,
    left: f64,
    state: State<'_, InstrumentEngine>,
    window_state: State<'_, crate::setup::WindowState>,
    app: AppHandle,
) -> Result<()> {
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
    state: tauri::State<'_, crate::instrument::engine::InstrumentEngine>,
) -> common::error::Result<common::instrument::data::StringSnoopDataResponse> {
    let samples = state.snapshot_output_snoop(NodeKey::new(group as u8, key as u8));
    Ok(common::instrument::data::StringSnoopDataResponse { samples })
}

#[tauri::command]
pub fn instrument_all_string_snoops(
    state: tauri::State<'_, crate::instrument::engine::InstrumentEngine>,
) -> common::error::Result<common::instrument::data::StringSnoopBatchPayload> {
    use std::time::{SystemTime, UNIX_EPOCH};

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
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    Ok(common::instrument::data::StringSnoopBatchPayload {
        t_unix_ms,
        snoops: entries,
    })
}

#[tauri::command]
pub fn instrument_excitement_snoop_data(
    group: usize,
    key: usize,
    state: tauri::State<'_, crate::instrument::engine::InstrumentEngine>,
) -> common::error::Result<common::instrument::data::ExcitementSnoopDataResponse> {
    let samples = state.snapshot_excitement_snoop(NodeKey::new(group as u8, key as u8));
    Ok(common::instrument::data::ExcitementSnoopDataResponse { samples })
}

#[tauri::command]
pub fn instrument_all_excitement_snoops(
    state: tauri::State<'_, crate::instrument::engine::InstrumentEngine>,
) -> common::error::Result<common::instrument::data::ExcitementSnoopBatchPayload> {
    use std::time::{SystemTime, UNIX_EPOCH};

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
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    Ok(common::instrument::data::ExcitementSnoopBatchPayload {
        t_unix_ms,
        snoops: entries,
    })
}

#[tauri::command]
/// Updates the band control value for a specific key
pub fn instrument_update_band_control(
    group: u8,
    key: u8,
    value: f32,
    state: State<'_, InstrumentEngine>,
    app: AppHandle,
) -> Result<()> {
    log::trace!(
        "instrument_update_band_control called: group={}, key={}, value={}",
        group,
        key,
        value
    );

    // Validate NodeKey against current layout
    let layout = state.layout();
    let registry = layout.registry();
    let node_key = match registry.create_key(group, key) {
        Ok(key) => key,
        Err(err) => {
            log::warn!("Invalid NodeKey in band control update: {}", err);
            return Err(common::error::ControlError::NodeNotFound {
                key: NodeKey::new(group, key),
            }
            .into());
        }
    };

    state.set_band_control(node_key, value)?;

    log::trace!(
        "Band control updated for node ({}, {}): value={}",
        group,
        key,
        value
    );

    app.emit(
        BAND_CONTROL_G_K,
        UpdateBandControlPayload { group, key, value },
    )?;

    Ok(())
}

/// Update key control state (pressed/released) for a specific key
#[tauri::command]
pub fn instrument_update_key_control(
    group: u8,
    key: u8,
    value: f32,
    state: State<'_, InstrumentEngine>,
    app: AppHandle,
) -> Result<()> {
    log::trace!(
        "instrument_update_key_control called: group={}, key={}, value={}",
        group,
        key,
        value
    );

    // Validate NodeKey against current layout
    let layout = state.layout();
    let registry = layout.registry();
    let node_key = match registry.create_key(group, key) {
        Ok(key) => key,
        Err(err) => {
            log::warn!("Invalid NodeKey in key control update: {}", err);
            return Err(common::error::ControlError::NodeNotFound {
                key: NodeKey::new(group, key),
            }
            .into());
        }
    };

    state.set_key_control(node_key, value)?;

    log::trace!(
        "Key control updated for node ({}, {}): value={} ({})",
        group,
        key,
        value,
        if value > 0.5 { "pressed" } else { "released" }
    );

    app.emit(
        KEY_CONTROL_G_K,
        UpdateKeyControlPayload { group, key, value },
    )?;

    Ok(())
}

#[tauri::command]
pub fn instrument_is_batch_processing(state: State<'_, InstrumentEngine>) -> bool {
    state.is_batch_processing()
}

#[cfg(feature="devtools")]
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn instrument_edit_finetuned_values(
    siren_alpha: f32,
    siren_beta: f32,
    siren_gamma: f32,
    group_ls_gain: f32,
    group_q: f32,
    node_follow_response_time_s: f32,
    filter_switch_follow_response_s: f32,
    filter_allpass_q: f32,
    filter_moog_q: f32,
    filter_shelf_q: f32,
    filter_shelf_gain: f32,
    filter_pass_q: f32,
    node_bell_q: f32,
    node_bell_gain_db: f32,
    formant_base_q: f32,
    input_ny_threshold: f32,
    input_ny_wet_ratio: f32,
    state: State<'_, InstrumentEngine>,
) -> Result<common::commands::edit::FineTunedValuesPayload> {
    let values = common::commands::edit::FineTunedValuesPayload{
        siren_alpha,
        siren_beta,
        siren_gamma,
        group_ls_gain,
        group_q,
        filter_switch_follow_response_s,
        node_follow_response_time_s,
        filter_allpass_q,
        filter_moog_q,
        filter_shelf_q,
        filter_shelf_gain,
        filter_pass_q,
        node_bell_q,
        node_bell_gain_db,
        formant_base_q,
        input_ny_threshold,
        input_ny_wet_ratio,
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
    state: State<'_, InstrumentEngine>,
) -> Result<common::commands::edit::FineTunedValuesPayload> {
    state.get_finetuned_values()
}
