//! Audio Worklet Event Constants and Payloads
//!
//! PURPOSE
//! -------
//! Define event names and payload structures for communication between
//! src-tauri (WebAudioController) and the hidden webview (audio-worklet.html).
//!
//! COMMUNICATION FLOW
//! ------------------
//! src-tauri emits these events → hidden webview → AudioWorklet
//!
//! MAYA DRY KISS
//! -------------
//! - Single source of truth for event names
//! - Simple, focused payload structures
//! - Clear naming convention: AUDIO_WORKLET_*

use serde::{Deserialize, Serialize};

// Event names (src-tauri → hidden webview)
pub const AUDIO_WORKLET_START: &str = "audio_worklet_start";
pub const AUDIO_WORKLET_STOP: &str = "audio_worklet_stop";
pub const AUDIO_WORKLET_PAUSE: &str = "audio_worklet_pause";
pub const AUDIO_WORKLET_RESUME: &str = "audio_worklet_resume";
pub const AUDIO_WORKLET_LAYOUT_CONFIG: &str = "audio_worklet_layout_config";
pub const AUDIO_WORKLET_ACTIVATION_SOURCE: &str = "audio_worklet_activation_source";
pub const AUDIO_WORKLET_SET_BAND_CONTROL: &str = "audio_worklet_set_band_control";
pub const AUDIO_WORKLET_SNOOP_REQUEST: &str = "audio_worklet_snoop_request";

// ========================================================================
// Event Payloads
// ========================================================================

/// Payload for starting DSP processing
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioWorkletStartPayload {
    /// JSON-serialized InstrumentLayout
    pub layout_json: String,
    /// JSON-serialized InstrumentConfig
    pub config_json: String,
    /// JSON-serialized TunerConfig
    pub tuner_config_json: String,
    /// Activation source: 0 = entropy, 1 = microphone
    pub activation_source: u8,
}

/// Payload for rebuilding networks when layout/config changes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioWorkletLayoutConfigPayload {
    /// JSON-serialized InstrumentLayout
    pub layout_json: String,
    /// JSON-serialized InstrumentConfig
    pub config_json: String,
    /// JSON-serialized TunerConfig
    pub tuner_config_json: String,
}

/// Payload for switching activation source
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioWorkletActivationSourcePayload {
    /// Activation source: 0 = entropy, 1 = microphone
    pub source: u8,
}

/// Payload for updating band control parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioWorkletSetBandControlPayload {
    /// Node group identifier
    pub group: u8,
    /// Node key identifier
    pub key: u8,
    /// Control parameter value
    pub value: f32,
}

/// Payload for requesting snoop data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioWorkletSnoopRequestPayload {
    /// Request type: "output" or "activation"
    pub snoop_type: String,
    /// Optional specific group (null for all)
    pub group: Option<u8>,
    /// Optional specific key (null for all)
    pub key: Option<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_worklet_payload_serialization() {
        // Test AudioWorkletStartPayload
        let start_payload = AudioWorkletStartPayload {
            layout_json: r#"{"scale":"Yo","space":[800.0,600.0]}"#.to_string(),
            config_json: r#"{"sample_rate":44100}"#.to_string(),
            tuner_config_json: r#"{"enabled":true}"#.to_string(),
            activation_source: 1,
        };

        let json = serde_json::to_string(&start_payload).unwrap();
        let deserialized: AudioWorkletStartPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.activation_source, 1);
        assert!(deserialized.layout_json.contains("Yo"));

        // Test AudioWorkletSetBandControlPayload
        let control_payload = AudioWorkletSetBandControlPayload {
            group: 0,
            key: 5,
            value: 0.75,
        };

        let json = serde_json::to_string(&control_payload).unwrap();
        let deserialized: AudioWorkletSetBandControlPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.group, 0);
        assert_eq!(deserialized.key, 5);
        assert_eq!(deserialized.value, 0.75);

        // Test AudioWorkletSnoopRequestPayload
        let snoop_payload = AudioWorkletSnoopRequestPayload {
            snoop_type: "output".to_string(),
            group: Some(2),
            key: None,
        };

        let json = serde_json::to_string(&snoop_payload).unwrap();
        let deserialized: AudioWorkletSnoopRequestPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.snoop_type, "output");
        assert_eq!(deserialized.group, Some(2));
        assert_eq!(deserialized.key, None);
    }

    #[test]
    fn test_event_constants() {
        // Verify event names follow expected naming convention
        assert_eq!(AUDIO_WORKLET_START, "audio_worklet_start");
        assert_eq!(AUDIO_WORKLET_STOP, "audio_worklet_stop");
        assert_eq!(AUDIO_WORKLET_PAUSE, "audio_worklet_pause");
        assert_eq!(AUDIO_WORKLET_RESUME, "audio_worklet_resume");
        assert_eq!(AUDIO_WORKLET_LAYOUT_CONFIG, "audio_worklet_layout_config");
        assert_eq!(
            AUDIO_WORKLET_ACTIVATION_SOURCE,
            "audio_worklet_activation_source"
        );
        assert_eq!(
            AUDIO_WORKLET_SET_BAND_CONTROL,
            "audio_worklet_set_band_control"
        );
        assert_eq!(AUDIO_WORKLET_SNOOP_REQUEST, "audio_worklet_snoop_request");
    }
}
