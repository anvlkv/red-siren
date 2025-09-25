use serde::{Deserialize, Serialize};

/// trigger navigation
pub const NAVIGATE: &str = "navigation_request";
/// confirm enter phase done
pub const NAV_ENTER_DONE: &str = "navigation_enter_done";
/// confirm leave phase done
pub const NAV_LEAVE_DONE: &str = "navigation_leave_done";
/// Request synchronization with backend route, typically after UI reloads.
pub const NAV_SYNC: &str = "navigation_sync";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigateRequestPayload {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavTxPayload {
    pub tx_id: u64,
}
