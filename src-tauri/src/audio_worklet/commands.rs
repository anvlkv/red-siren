//! Audio Worklet Command Handlers
//!
//! PURPOSE
//! -------
//! Tauri command handlers that receive responses from the hidden webview
//! after it processes audio worklet operations. Updates shared state that
//! WebAudioController waits on.
//!
//! COMMUNICATION FLOW
//! ------------------
//! hidden webview → tauri.invoke(command) → these handlers → update WorkletState
//! WebAudioController reads results from WorkletState
//!
//! MAYA DRY KISS
//! -------------
//! - Simple command handlers focused on state updates
//! - Clear error handling and logging
//! - Direct integration with WorkletState

use std::sync::Arc;
use tauri::{AppHandle, State};

use common::commands::audio_worklet::{
    AudioWorkletAckPayload, AudioWorkletMicPermissionPayload, AudioWorkletSnoopResponsePayload,
};
use common::error::Result;

use super::state::WorkletState;

/// Receive acknowledgement from hidden webview after completing an operation
#[tauri::command]
pub async fn audio_worklet_ack(
    _app: AppHandle,
    state: State<'_, Arc<WorkletState>>,
    request_id: String,
    payload: AudioWorkletAckPayload,
) -> Result<()> {
    log::debug!(
        "Received ack for request {}: success={}, op={}",
        request_id,
        payload.success,
        payload.operation
    );

    // Update the shared state with the acknowledgement result
    state.update_ack(&request_id, payload);

    Ok(())
}

/// Receive snoop data response from hidden webview and store in shared state
#[tauri::command]
pub async fn audio_worklet_snoop_response(
    _app: AppHandle,
    state: State<'_, Arc<WorkletState>>,
    request_id: String,
    payload: AudioWorkletSnoopResponsePayload,
) -> Result<()> {
    log::debug!(
        "Received snoop response for request {}: type={}, single_node={}",
        request_id,
        payload.snoop_type,
        payload.is_single_node
    );

    // Log sample count for debugging
    if payload.is_single_node {
        if let Some(ref node_data) = payload.single_node_data {
            log::trace!(
                "Single node data: group={}, key={}, samples={}",
                node_data.group,
                node_data.key,
                node_data.samples.len()
            );
        }
    } else if let Some(ref all_data) = payload.all_nodes_data {
        log::trace!("All nodes data: {} nodes", all_data.len());
    }

    // Update the shared state with the snoop response
    state.update_snoop_response(&request_id, payload);

    Ok(())
}

/// Request microphone permission from browser and return result
#[tauri::command]
pub async fn audio_worklet_mic_permission_check(
    _app: AppHandle,
    state: State<'_, Arc<WorkletState>>,
    payload: AudioWorkletMicPermissionPayload,
) -> Result<bool> {
    log::info!(
        "Microphone permission check result: granted={}",
        payload.granted
    );

    if let Some(ref error_msg) = payload.error_message {
        log::warn!("Microphone permission error: {}", error_msg);
    }

    // Update the shared state with the permission result
    state.set_mic_permission(payload.granted);

    Ok(payload.granted)
}

/// Helper command to check if audio worklet is supported (always true for web runtime)
#[tauri::command]
pub async fn audio_worklet_supports_mic(_app: AppHandle) -> Result<bool> {
    // Web runtime always supports microphone (subject to browser permission)
    Ok(true)
}

/// Get current microphone permission status from shared state
#[tauri::command]
pub async fn audio_worklet_get_mic_permission(
    _app: AppHandle,
    state: State<'_, Arc<WorkletState>>,
) -> Result<Option<bool>> {
    let permission = state.get_mic_permission();
    log::debug!("Current mic permission: {:?}", permission);
    Ok(permission)
}

/// Trigger cleanup of expired operations in shared state
#[tauri::command]
pub async fn audio_worklet_cleanup_expired(
    _app: AppHandle,
    state: State<'_, Arc<WorkletState>>,
) -> Result<()> {
    log::trace!("Cleaning up expired audio worklet operations");
    state.cleanup_expired();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_state() -> Arc<WorkletState> {
        WorkletState::new()
    }

    #[test]
    fn test_worklet_state_creation() {
        let state = create_test_state();
        // Just verify we can create the state without panicking
        assert!(state.pending_acks.lock().is_empty());
        assert!(state.snoop_responses.lock().is_empty());
    }

    // TODO: Re-enable full tests once proper testing infrastructure is set up
    // Tests temporarily disabled due to Tauri mock framework issues
}
