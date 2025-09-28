use serde::{Deserialize, Serialize};

/// trigger navigation
pub const NAVIGATE: &str = "navigation_request";
/// confirm enter phase done
pub const NAV_ENTER_DONE: &str = "navigation_enter_done";
/// confirm leave phase done
pub const NAV_LEAVE_DONE: &str = "navigation_leave_done";
/// Request synchronization with backend route, typically after UI reloads.
pub const NAV_SYNC: &str = "navigation_sync";
/// Bootstrap initial navigation transaction (tx_id=0) after listeners are mounted.
pub const NAV_BOOTSTRAP: &str = "navigation_bootstrap";
/// Continue previously gated navigation (after async gating resolves).
pub const NAV_RESUME: &str = "navigation_resume";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavSyncRequestPayload {
    pub route: crate::navigation::RouteId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigateRequestPayload {
    pub route: crate::navigation::RouteId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavTxPayload {
    pub tx_id: u64,
}
