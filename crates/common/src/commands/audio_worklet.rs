//! Audio Worklet Command Constants and Payloads
//!
//! PURPOSE
//! -------
//! Define command names and payload structures for communication from
//! the hidden webview back to src-tauri (acknowledgements and responses).
//!
//! COMMUNICATION FLOW
//! ------------------
//! hidden webview → src-tauri (via tauri.invoke calls)
//!
//! MAYA DRY KISS
//! -------------
//! - Single source of truth for command names
//! - Simple response structures
//! - Clear naming convention: AUDIO_WORKLET_*

use serde::{Deserialize, Serialize};

// Command names (hidden webview → src-tauri)
pub const AUDIO_WORKLET_ACK: &str = "audio_worklet_ack";
pub const AUDIO_WORKLET_SNOOP_RESPONSE: &str = "audio_worklet_snoop_response";
pub const AUDIO_WORKLET_MIC_PERMISSION_CHECK: &str = "audio_worklet_mic_permission_check";

// ========================================================================
// Command Payloads
// ========================================================================

/// Acknowledgement payload for operation completion
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioWorkletAckPayload {
    /// Operation that was completed
    pub operation: String,
    /// Whether the operation succeeded
    pub success: bool,
    /// Optional error message if operation failed
    pub error_message: Option<String>,
}

/// Response payload for snoop data requests
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioWorkletSnoopResponsePayload {
    /// Type of snoop data: "output" or "activation"
    pub snoop_type: String,
    /// Whether this is for a specific node or all nodes
    pub is_single_node: bool,
    /// Single node data (if is_single_node = true)
    pub single_node_data: Option<SnoopNodeData>,
    /// All nodes data (if is_single_node = false)
    pub all_nodes_data: Option<Vec<SnoopNodeData>>,
}

/// Individual node snoop data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnoopNodeData {
    /// Node group identifier
    pub group: u8,
    /// Node key identifier
    pub key: u8,
    /// Sample buffer data
    pub samples: Vec<f32>,
}

/// Mic permission check result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioWorkletMicPermissionPayload {
    /// Whether microphone permission is granted
    pub granted: bool,
    /// Optional error message if permission check failed
    pub error_message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_worklet_command_payload_serialization() {
        // Test AudioWorkletAckPayload
        let ack_payload = AudioWorkletAckPayload {
            operation: "start".to_string(),
            success: true,
            error_message: None,
        };

        let json = serde_json::to_string(&ack_payload).unwrap();
        let deserialized: AudioWorkletAckPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.operation, "start");
        assert!(deserialized.success);
        assert_eq!(deserialized.error_message, None);

        // Test error case
        let error_payload = AudioWorkletAckPayload {
            operation: "rebuild_networks".to_string(),
            success: false,
            error_message: Some("Invalid layout JSON".to_string()),
        };

        let json = serde_json::to_string(&error_payload).unwrap();
        let deserialized: AudioWorkletAckPayload = serde_json::from_str(&json).unwrap();
        assert!(!deserialized.success);
        assert_eq!(
            deserialized.error_message,
            Some("Invalid layout JSON".to_string())
        );

        // Test AudioWorkletSnoopResponsePayload with single node
        let snoop_data = SnoopNodeData {
            group: 1,
            key: 3,
            samples: vec![0.1, 0.2, -0.1, 0.0],
        };

        let snoop_response = AudioWorkletSnoopResponsePayload {
            snoop_type: "output".to_string(),
            is_single_node: true,
            single_node_data: Some(snoop_data.clone()),
            all_nodes_data: None,
        };

        let json = serde_json::to_string(&snoop_response).unwrap();
        let deserialized: AudioWorkletSnoopResponsePayload = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.snoop_type, "output");
        assert!(deserialized.is_single_node);
        assert!(deserialized.single_node_data.is_some());
        let node_data = deserialized.single_node_data.unwrap();
        assert_eq!(node_data.group, 1);
        assert_eq!(node_data.key, 3);
        assert_eq!(node_data.samples.len(), 4);

        // Test AudioWorkletMicPermissionPayload
        let mic_payload = AudioWorkletMicPermissionPayload {
            granted: false,
            error_message: Some("Permission denied by user".to_string()),
        };

        let json = serde_json::to_string(&mic_payload).unwrap();
        let deserialized: AudioWorkletMicPermissionPayload = serde_json::from_str(&json).unwrap();
        assert!(!deserialized.granted);
        assert!(deserialized.error_message.is_some());
    }

    #[test]
    fn test_command_constants() {
        // Verify command names follow expected naming convention
        assert_eq!(AUDIO_WORKLET_ACK, "audio_worklet_ack");
        assert_eq!(AUDIO_WORKLET_SNOOP_RESPONSE, "audio_worklet_snoop_response");
        assert_eq!(
            AUDIO_WORKLET_MIC_PERMISSION_CHECK,
            "audio_worklet_mic_permission_check"
        );
    }
}
