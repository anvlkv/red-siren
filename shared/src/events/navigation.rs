pub const NAV_REQUESTED: &str = "navigation_requested";
pub const NAV_GATED: &str = "navigation_gated";
pub const NAV_STARTED: &str = "navigation_started";
pub const NAV_COMMITTED: &str = "navigation_committed";
pub const NAV_COMPLETED: &str = "navigation_completed";
pub const NAV_CANCELED: &str = "navigation_canceled";
/// Synchronize UI with backend route, typically after UI reloads.
/// Payload: NavSyncPayload { to, path }
pub const NAV_SYNC: &str = "navigation_sync";

/// Payload type for navigation_sync.
/// Emitted via backend snapshot to align router after reloads.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavSyncPayload {
    pub to: crate::RouteId,
    pub path: String,
}
