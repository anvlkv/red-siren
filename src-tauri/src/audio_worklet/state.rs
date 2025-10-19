//! Shared State Management for Audio Worklet Communication
//!
//! PURPOSE
//! -------
//! Manages coordination between WebAudioController emit operations and
//! command handler responses from the hidden webview. Provides timeout
//! handling and result storage for async operations.
//!
//! ARCHITECTURE
//! ------------
//! WebAudioController emits event → waits on shared state
//! Hidden webview processes → invokes command → updates shared state
//! WebAudioController reads result from shared state
//!
//! MAYA DRY KISS
//! -------------
//! - Simple state management with parking_lot::Mutex
//! - Clear timeout handling with tokio
//! - Focused result types for each operation

use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use common::commands::audio_worklet::{
    AudioWorkletAckPayload, AudioWorkletSnoopResponsePayload, SnoopNodeData,
};

/// Operation timeout duration (500ms as specified in plan)
const OPERATION_TIMEOUT: Duration = Duration::from_millis(500);

/// Shared state for coordinating audio worklet operations
#[derive(Debug, Default)]
pub struct WorkletState {
    /// Pending acknowledgements keyed by operation name
    pub pending_acks: Mutex<HashMap<String, AckResult>>,
    /// Snoop response data storage
    pub snoop_responses: Mutex<HashMap<String, SnoopResponseResult>>,
    /// Microphone permission check results
    pub mic_permission: Mutex<Option<bool>>,
}

/// Result of an acknowledgement operation
#[derive(Debug, Clone)]
pub struct AckResult {
    /// Whether the operation completed (success or failure)
    pub completed: bool,
    /// Whether the operation succeeded
    pub success: bool,
    /// Optional error message if operation failed
    pub error_message: Option<String>,
    /// Timestamp when the operation was initiated
    pub initiated_at: Instant,
}

/// Result of a snoop data request
#[derive(Debug, Clone)]
pub struct SnoopResponseResult {
    /// Whether the response has been received
    pub received: bool,
    /// The snoop response data
    pub data: Option<AudioWorkletSnoopResponsePayload>,
    /// Timestamp when the request was initiated
    pub initiated_at: Instant,
}

impl WorkletState {
    /// Initialize a new worklet state
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Initiate a new operation and return a request ID
    pub fn initiate_operation(&self, operation: &str) -> String {
        let request_id = format!("{}_{}", operation, Instant::now().elapsed().as_nanos());

        let ack_result = AckResult {
            completed: false,
            success: false,
            error_message: None,
            initiated_at: Instant::now(),
        };

        self.pending_acks
            .lock()
            .insert(request_id.clone(), ack_result);
        request_id
    }

    /// Wait for an operation acknowledgement with timeout
    pub fn wait_for_ack(&self, request_id: &str) -> Result<bool, WorkletError> {
        let start_time = Instant::now();

        loop {
            // Check if we've exceeded timeout
            if start_time.elapsed() > OPERATION_TIMEOUT {
                // Clean up the pending request
                self.pending_acks.lock().remove(request_id);
                return Err(WorkletError::Timeout {
                    operation: request_id.to_string(),
                });
            }

            // Check if the operation completed
            {
                let pending = self.pending_acks.lock();
                if let Some(result) = pending.get(request_id) {
                    if result.completed {
                        let success = result.success;
                        let error_message = result.error_message.clone();

                        // Clean up the completed request
                        drop(pending);
                        self.pending_acks.lock().remove(request_id);

                        if success {
                            return Ok(true);
                        } else {
                            return Err(WorkletError::OperationFailed {
                                operation: request_id.to_string(),
                                message: error_message
                                    .unwrap_or_else(|| "Unknown error".to_string()),
                            });
                        }
                    }
                }
            }

            // Sleep briefly before checking again
            thread::sleep(Duration::from_millis(10));
        }
    }

    /// Update acknowledgement result from command handler
    pub fn update_ack(&self, request_id: &str, payload: AudioWorkletAckPayload) {
        let mut pending = self.pending_acks.lock();
        if let Some(result) = pending.get_mut(request_id) {
            result.completed = true;
            result.success = payload.success;
            result.error_message = payload.error_message;
        }
    }

    /// Initiate a snoop request and return a request ID
    pub fn initiate_snoop_request(
        &self,
        snoop_type: &str,
        group: Option<u8>,
        key: Option<u8>,
    ) -> String {
        let request_id = format!(
            "snoop_{}_{}_{}_{}",
            snoop_type,
            group
                .map(|g| g.to_string())
                .unwrap_or_else(|| "all".to_string()),
            key.map(|k| k.to_string())
                .unwrap_or_else(|| "all".to_string()),
            Instant::now().elapsed().as_nanos()
        );

        let snoop_result = SnoopResponseResult {
            received: false,
            data: None,
            initiated_at: Instant::now(),
        };

        self.snoop_responses
            .lock()
            .insert(request_id.clone(), snoop_result);
        request_id
    }

    /// Wait for a snoop response with timeout
    pub fn wait_for_snoop_response(
        &self,
        request_id: &str,
    ) -> Result<AudioWorkletSnoopResponsePayload, WorkletError> {
        let start_time = Instant::now();

        loop {
            // Check if we've exceeded timeout
            if start_time.elapsed() > OPERATION_TIMEOUT {
                // Clean up the pending request
                self.snoop_responses.lock().remove(request_id);
                return Err(WorkletError::Timeout {
                    operation: request_id.to_string(),
                });
            }

            // Check if the response was received
            {
                let responses = self.snoop_responses.lock();
                if let Some(result) = responses.get(request_id) {
                    if result.received {
                        if let Some(data) = &result.data {
                            let response_data = data.clone();

                            // Clean up the completed request
                            drop(responses);
                            self.snoop_responses.lock().remove(request_id);

                            return Ok(response_data);
                        }
                    }
                }
            }

            // Sleep briefly before checking again
            thread::sleep(Duration::from_millis(10));
        }
    }

    /// Update snoop response from command handler
    pub fn update_snoop_response(
        &self,
        request_id: &str,
        payload: AudioWorkletSnoopResponsePayload,
    ) {
        let mut responses = self.snoop_responses.lock();
        if let Some(result) = responses.get_mut(request_id) {
            result.received = true;
            result.data = Some(payload);
        }
    }

    /// Set microphone permission result
    pub fn set_mic_permission(&self, granted: bool) {
        *self.mic_permission.lock() = Some(granted);
    }

    /// Get current microphone permission status
    pub fn get_mic_permission(&self) -> Option<bool> {
        *self.mic_permission.lock()
    }

    /// Clean up expired pending operations (called periodically)
    pub fn cleanup_expired(&self) {
        let now = Instant::now();

        // Clean up expired acknowledgements
        {
            let mut pending = self.pending_acks.lock();
            pending.retain(|_id, result| {
                now.duration_since(result.initiated_at) < OPERATION_TIMEOUT * 2
            });
        }

        // Clean up expired snoop requests
        {
            let mut responses = self.snoop_responses.lock();
            responses.retain(|_id, result| {
                now.duration_since(result.initiated_at) < OPERATION_TIMEOUT * 2
            });
        }
    }
}

/// Errors that can occur during worklet state operations
#[derive(Debug)]
pub enum WorkletError {
    /// Operation timed out
    Timeout { operation: String },

    /// Operation failed with error message
    OperationFailed { operation: String, message: String },

    /// Invalid request ID
    InvalidRequestId { request_id: String },
}

impl std::fmt::Display for WorkletError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkletError::Timeout { operation } => write!(f, "operation timed out: {}", operation),
            WorkletError::OperationFailed { operation, message } => {
                write!(f, "operation failed ({}): {}", operation, message)
            }
            WorkletError::InvalidRequestId { request_id } => {
                write!(f, "invalid request id: {}", request_id)
            }
        }
    }
}

impl std::error::Error for WorkletError {}

/// Helper function to extract operation name from request ID
pub fn extract_operation_name(request_id: &str) -> &str {
    request_id.split('_').next().unwrap_or(request_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ack_workflow() {
        let state = WorkletState::new();
        let request_id = state.initiate_operation("start");

        // Simulate command handler updating the ack
        let ack_payload = AudioWorkletAckPayload {
            operation: "start".to_string(),
            success: true,
            error_message: None,
        };
        state.update_ack(&request_id, ack_payload);

        // Wait for ack should succeed
        let result = state.wait_for_ack(&request_id);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_ack_failure() {
        let state = WorkletState::new();
        let request_id = state.initiate_operation("start");

        // Simulate failed operation
        let ack_payload = AudioWorkletAckPayload {
            operation: "start".to_string(),
            success: false,
            error_message: Some("WASM module not loaded".to_string()),
        };
        state.update_ack(&request_id, ack_payload);

        // Wait for ack should return error
        let result = state.wait_for_ack(&request_id);
        assert!(result.is_err());
        if let Err(WorkletError::OperationFailed { message, .. }) = result {
            assert_eq!(message, "WASM module not loaded");
        }
    }

    #[test]
    fn test_snoop_workflow() {
        let state = WorkletState::new();
        let request_id = state.initiate_snoop_request("output", Some(0), Some(1));

        // Simulate snoop response
        let snoop_data = SnoopNodeData {
            group: 0,
            key: 1,
            samples: vec![0.1, -0.1, 0.2, -0.2],
        };
        let snoop_payload = AudioWorkletSnoopResponsePayload {
            snoop_type: "output".to_string(),
            is_single_node: true,
            single_node_data: Some(snoop_data),
            all_nodes_data: None,
        };
        state.update_snoop_response(&request_id, snoop_payload);

        // Wait for response should succeed
        let result = state.wait_for_snoop_response(&request_id);
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.snoop_type, "output");
        assert!(response.is_single_node);
        assert!(response.single_node_data.is_some());
    }

    #[test]
    fn test_mic_permission() {
        let state = WorkletState::new();

        // Initially no permission set
        assert_eq!(state.get_mic_permission(), None);

        // Set permission
        state.set_mic_permission(true);
        assert_eq!(state.get_mic_permission(), Some(true));

        // Change permission
        state.set_mic_permission(false);
        assert_eq!(state.get_mic_permission(), Some(false));
    }

    #[test]
    fn test_cleanup_expired() {
        let state = WorkletState::new();
        let request_id = state.initiate_operation("test");

        // Manually set old timestamp to simulate expired operation
        {
            let mut pending = state.pending_acks.lock();
            if let Some(result) = pending.get_mut(&request_id) {
                result.initiated_at = Instant::now() - OPERATION_TIMEOUT * 3;
            }
        }

        // Cleanup should remove expired operations
        state.cleanup_expired();

        let pending = state.pending_acks.lock();
        assert!(!pending.contains_key(&request_id));
    }
}
