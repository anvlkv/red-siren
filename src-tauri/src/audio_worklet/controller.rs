//! WebAudioController - StreamController Implementation for Web Audio API
//!
//! PURPOSE
//! -------
//! Implements the StreamController trait using Web Audio API + AudioWorklet
//! through communication with a hidden webview. Replaces the CPAL-based
//! audio runtime for web deployment scenarios.
//!
//! ARCHITECTURE
//! ------------
//! WebAudioController (src-tauri) → app.emit(event) → hidden webview
//! Hidden webview → AudioWorklet → WASM processing → tauri.invoke(response)
//! Command handlers → update WorkletState → WebAudioController continues
//!
//! MAYA DRY KISS
//! -------------
//! - Clean StreamController implementation
//! - Simple emit/wait coordination pattern
//! - Focused error handling with timeouts

use std::sync::Arc;
use tauri::{AppHandle, Emitter};

use common::audio::{ActivationSource, StreamController};
use common::error::{InstrumentError, Result};
use common::events::audio_worklet::{
    AudioWorkletActivationSourcePayload, AudioWorkletLayoutConfigPayload,
    AudioWorkletSetBandControlPayload, AudioWorkletSnoopRequestPayload, AudioWorkletStartPayload,
    AUDIO_WORKLET_ACTIVATION_SOURCE, AUDIO_WORKLET_LAYOUT_CONFIG, AUDIO_WORKLET_PAUSE,
    AUDIO_WORKLET_RESUME, AUDIO_WORKLET_SET_BAND_CONTROL, AUDIO_WORKLET_SNOOP_REQUEST,
    AUDIO_WORKLET_START, AUDIO_WORKLET_STOP,
};
use common::instrument::{Config as InstrumentConfig, Layout as InstrumentLayout};
use common::tuner::Config as TunerConfig;

use super::state::{WorkletError, WorkletState};

/// WebAudioController implementing StreamController trait
pub struct WebAudioController {
    /// Tauri app handle for emitting events to hidden webview
    app: AppHandle,
    /// Shared state for coordinating acknowledgements from invoke calls
    state: Arc<WorkletState>,
}

impl WebAudioController {
    /// Create a new WebAudioController
    pub fn new(app: AppHandle, state: Arc<WorkletState>) -> Self {
        Self { app, state }
    }
}

impl StreamController for WebAudioController {
    /// Start DSP processing by building Fundsp networks from layout/config
    fn start(
        &self,
        layout: &InstrumentLayout,
        config: &InstrumentConfig,
        source: ActivationSource,
        tuner_config: &TunerConfig,
    ) -> Result<()> {
        log::info!(
            "Starting WebAudio DSP processing with activation source: {:?}",
            source
        );

        // Serialize configurations to JSON
        let layout_json =
            serde_json::to_string(layout).map_err(|e| InstrumentError::StartFailed {
                detail: Some(format!("Failed to serialize layout: {}", e)),
            })?;

        let config_json =
            serde_json::to_string(config).map_err(|e| InstrumentError::StartFailed {
                detail: Some(format!("Failed to serialize config: {}", e)),
            })?;

        let tuner_config_json =
            serde_json::to_string(tuner_config).map_err(|e| InstrumentError::StartFailed {
                detail: Some(format!("Failed to serialize tuner config: {}", e)),
            })?;

        // Create start payload
        let payload = AudioWorkletStartPayload {
            layout_json,
            config_json,
            tuner_config_json,
            activation_source: source.into(),
        };

        // Initiate operation and get request ID
        let request_id = self.state.initiate_operation("start");

        // Emit event to hidden webview
        self.app
            .emit_to(
                "audio-worklet",
                AUDIO_WORKLET_START,
                (&request_id, &payload),
            )
            .map_err(|e| InstrumentError::StartFailed {
                detail: Some(format!("Failed to emit start event: {}", e)),
            })?;

        // Wait for acknowledgement
        self.state.wait_for_ack(&request_id).map_err(|e| match e {
            WorkletError::Timeout { operation } => InstrumentError::AckTimeout { op: operation },
            WorkletError::OperationFailed { message, .. } => InstrumentError::StartFailed {
                detail: Some(message),
            },
            _ => InstrumentError::StartFailed {
                detail: Some("Unknown error".to_string()),
            },
        })?;

        log::info!("WebAudio DSP processing started successfully");
        Ok(())
    }

    /// Stop DSP processing and clean up resources
    fn stop(&self) -> Result<()> {
        log::info!("Stopping WebAudio DSP processing");

        let request_id = self.state.initiate_operation("stop");

        // Emit stop event (no payload needed)
        self.app
            .emit_to("audio-worklet", AUDIO_WORKLET_STOP, &request_id)
            .map_err(|e| InstrumentError::BackendMissing {
                op: format!("stop: {}", e),
            })?;

        // Wait for acknowledgement
        self.state.wait_for_ack(&request_id).map_err(|e| match e {
            WorkletError::Timeout { operation } => InstrumentError::AckTimeout { op: operation },
            _ => InstrumentError::BackendMissing {
                op: "stop".to_string(),
            },
        })?;

        log::info!("WebAudio DSP processing stopped");
        Ok(())
    }

    /// Pause DSP processing without destroying state
    fn pause(&self) -> Result<()> {
        log::info!("Pausing WebAudio DSP processing");

        let request_id = self.state.initiate_operation("pause");

        self.app
            .emit_to("audio-worklet", AUDIO_WORKLET_PAUSE, &request_id)
            .map_err(|e| InstrumentError::PauseFailed {
                detail: Some(format!("Failed to emit pause event: {}", e)),
            })?;

        self.state.wait_for_ack(&request_id).map_err(|e| match e {
            WorkletError::Timeout { operation } => InstrumentError::AckTimeout { op: operation },
            _ => InstrumentError::PauseFailed {
                detail: Some("Operation failed".to_string()),
            },
        })?;

        log::info!("WebAudio DSP processing paused");
        Ok(())
    }

    /// Resume DSP processing from paused state
    fn resume(&self) -> Result<()> {
        log::info!("Resuming WebAudio DSP processing");

        let request_id = self.state.initiate_operation("resume");

        self.app
            .emit_to("audio-worklet", AUDIO_WORKLET_RESUME, &request_id)
            .map_err(|e| InstrumentError::ResumeFailed {
                detail: Some(format!("Failed to emit resume event: {}", e)),
            })?;

        self.state.wait_for_ack(&request_id).map_err(|e| match e {
            WorkletError::Timeout { operation } => InstrumentError::AckTimeout { op: operation },
            _ => InstrumentError::ResumeFailed {
                detail: Some("Operation failed".to_string()),
            },
        })?;

        log::info!("WebAudio DSP processing resumed");
        Ok(())
    }

    /// Switch activation source between entropy and microphone input
    fn on_activation_source_changed(&self, source: ActivationSource) -> Result<()> {
        log::info!("Changing activation source to: {:?}", source);

        let payload = AudioWorkletActivationSourcePayload {
            source: source.into(),
        };

        let request_id = self.state.initiate_operation("activation_source");

        self.app
            .emit_to(
                "audio-worklet",
                AUDIO_WORKLET_ACTIVATION_SOURCE,
                (&request_id, &payload),
            )
            .map_err(|_e| InstrumentError::UnsupportedActivationSource(source.into()))?;

        self.state.wait_for_ack(&request_id).map_err(|e| match e {
            WorkletError::Timeout { .. } => {
                InstrumentError::UnsupportedActivationSource(source.into())
            }
            _ => InstrumentError::UnsupportedActivationSource(source.into()),
        })?;

        log::info!("Activation source changed successfully");
        Ok(())
    }

    /// Rebuild Fundsp networks when layout/config changes
    fn on_layout_changed(
        &self,
        layout: &InstrumentLayout,
        config: &InstrumentConfig,
        tuner_config: &TunerConfig,
    ) -> Result<()> {
        log::info!("Rebuilding networks for layout change");

        // Serialize configurations to JSON
        let layout_json =
            serde_json::to_string(layout).map_err(|e| InstrumentError::StartFailed {
                detail: Some(format!("Layout serialization failed: {}", e)),
            })?;

        let config_json =
            serde_json::to_string(config).map_err(|e| InstrumentError::StartFailed {
                detail: Some(format!("Config serialization failed: {}", e)),
            })?;

        let tuner_config_json =
            serde_json::to_string(tuner_config).map_err(|e| InstrumentError::StartFailed {
                detail: Some(format!("Tuner config serialization failed: {}", e)),
            })?;

        let payload = AudioWorkletLayoutConfigPayload {
            layout_json,
            config_json,
            tuner_config_json,
        };

        let request_id = self.state.initiate_operation("layout_config");

        self.app
            .emit_to(
                "audio-worklet",
                AUDIO_WORKLET_LAYOUT_CONFIG,
                (&request_id, &payload),
            )
            .map_err(|e| InstrumentError::StartFailed {
                detail: Some(format!("Failed to emit layout config: {}", e)),
            })?;

        self.state.wait_for_ack(&request_id).map_err(|e| match e {
            WorkletError::Timeout { operation } => InstrumentError::AckTimeout { op: operation },
            _ => InstrumentError::StartFailed {
                detail: Some("Layout change failed".to_string()),
            },
        })?;

        log::info!("Network rebuild completed successfully");
        Ok(())
    }

    /// Return current sample buffer from output snoop for specific node
    fn snapshot_output_snoop(&self, group: usize, key: usize) -> Vec<f32> {
        let payload = AudioWorkletSnoopRequestPayload {
            snoop_type: "output".to_string(),
            group: Some(group as u8),
            key: Some(key as u8),
        };

        let request_id =
            self.state
                .initiate_snoop_request("output", Some(group as u8), Some(key as u8));

        // Emit snoop request
        if let Err(e) = self.app.emit_to(
            "audio-worklet",
            AUDIO_WORKLET_SNOOP_REQUEST,
            (&request_id, &payload),
        ) {
            log::warn!("Failed to emit snoop request: {}", e);
            return Vec::new();
        }

        // Wait for response
        let response = self.state.wait_for_snoop_response(&request_id);

        match response {
            Ok(snoop_response) => {
                if let Some(node_data) = snoop_response.single_node_data {
                    node_data.samples
                } else {
                    log::warn!("Expected single node data but got none");
                    Vec::new()
                }
            }
            Err(e) => {
                log::warn!("Failed to get output snoop data: {:?}", e);
                Vec::new()
            }
        }
    }

    /// Return JSON array of all output snoop samples with group/key identifiers
    fn snapshot_all_output_snoops(&self) -> Vec<(u8, u8, Vec<f32>)> {
        let payload = AudioWorkletSnoopRequestPayload {
            snoop_type: "output".to_string(),
            group: None,
            key: None,
        };

        let request_id = self.state.initiate_snoop_request("output", None, None);

        // Emit snoop request
        if let Err(e) = self.app.emit_to(
            "audio-worklet",
            AUDIO_WORKLET_SNOOP_REQUEST,
            (&request_id, &payload),
        ) {
            log::warn!("Failed to emit all output snoop request: {}", e);
            return Vec::new();
        }

        // Wait for response
        let response = self.state.wait_for_snoop_response(&request_id);

        match response {
            Ok(snoop_response) => {
                if let Some(all_data) = snoop_response.all_nodes_data {
                    all_data
                        .into_iter()
                        .map(|node_data| (node_data.group, node_data.key, node_data.samples))
                        .collect()
                } else {
                    log::warn!("Expected all nodes data but got none");
                    Vec::new()
                }
            }
            Err(e) => {
                log::warn!("Failed to get all output snoop data: {:?}", e);
                Vec::new()
            }
        }
    }

    /// Return current sample buffer from activation snoop for specific node
    fn snapshot_activation_snoop(&self, group: usize, key: usize) -> Vec<f32> {
        let payload = AudioWorkletSnoopRequestPayload {
            snoop_type: "activation".to_string(),
            group: Some(group as u8),
            key: Some(key as u8),
        };

        let request_id =
            self.state
                .initiate_snoop_request("activation", Some(group as u8), Some(key as u8));

        // Emit snoop request
        if let Err(e) = self.app.emit_to(
            "audio-worklet",
            AUDIO_WORKLET_SNOOP_REQUEST,
            (&request_id, &payload),
        ) {
            log::warn!("Failed to emit activation snoop request: {}", e);
            return Vec::new();
        }

        // Wait for response
        let response = self.state.wait_for_snoop_response(&request_id);

        match response {
            Ok(snoop_response) => {
                if let Some(node_data) = snoop_response.single_node_data {
                    node_data.samples
                } else {
                    log::warn!("Expected single node data but got none");
                    Vec::new()
                }
            }
            Err(e) => {
                log::warn!("Failed to get activation snoop data: {:?}", e);
                Vec::new()
            }
        }
    }

    /// Return JSON array of all activation snoop samples with group/key identifiers
    fn snapshot_all_activation_snoops(&self) -> Vec<(u8, u8, Vec<f32>)> {
        let payload = AudioWorkletSnoopRequestPayload {
            snoop_type: "activation".to_string(),
            group: None,
            key: None,
        };

        let request_id = self.state.initiate_snoop_request("activation", None, None);

        // Emit snoop request
        if let Err(e) = self.app.emit_to(
            "audio-worklet",
            AUDIO_WORKLET_SNOOP_REQUEST,
            (&request_id, &payload),
        ) {
            log::warn!("Failed to emit all activation snoop request: {}", e);
            return Vec::new();
        }

        // Wait for response
        let response = self.state.wait_for_snoop_response(&request_id);

        match response {
            Ok(snoop_response) => {
                if let Some(all_data) = snoop_response.all_nodes_data {
                    all_data
                        .into_iter()
                        .map(|node_data| (node_data.group, node_data.key, node_data.samples))
                        .collect()
                } else {
                    log::warn!("Expected all nodes data but got none");
                    Vec::new()
                }
            }
            Err(e) => {
                log::warn!("Failed to get all activation snoop data: {:?}", e);
                Vec::new()
            }
        }
    }

    /// Update band control parameters for specific nodes
    fn set_band_control(&self, group: u8, key: u8, value: f32) -> Result<()> {
        log::debug!(
            "Setting band control: group={}, key={}, value={}",
            group,
            key,
            value
        );

        let payload = AudioWorkletSetBandControlPayload { group, key, value };

        let request_id = self.state.initiate_operation("set_band_control");

        self.app
            .emit_to(
                "audio-worklet",
                AUDIO_WORKLET_SET_BAND_CONTROL,
                (&request_id, &payload),
            )
            .map_err(|e| {
                InstrumentError::Control(common::error::ControlError::BackendMissing {
                    op: format!("set_band_control: {}", e),
                })
            })?;

        self.state.wait_for_ack(&request_id).map_err(|e| match e {
            WorkletError::Timeout { operation } => InstrumentError::AckTimeout { op: operation },
            _ => InstrumentError::Control(common::error::ControlError::BackendMissing {
                op: "set_band_control".to_string(),
            }),
        })?;

        log::debug!("Band control updated successfully");
        Ok(())
    }
}
