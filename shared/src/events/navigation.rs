/// Phase 2 Navigation Event Constants
///
/// Replaces legacy transitional events:
/// - PREPARE_TO_LEAVE
/// - PREPARE_TO_GO_TO
/// - GO_TO
/// - NAVIGATED
///
/// New model introduces a clearer lifecycle with gating + cancellation.
/// All payloads are expected to be JSON objects carrying at minimum a `tx_id`
/// and relevant route identifiers (variant name) plus auxiliary fields.
///
/// Emission Guidelines:
/// - REQUESTED: immediately when a navigation intent is received (before async gating)
/// - GATED: after gating completes (always emitted; `allowed: bool`)
/// - STARTED: first phase after gating success (entering Leaving phase, placeholder now)
/// - COMMITTED: router path is actually changed (point of no return)
/// - COMPLETED: post-commit stabilization finished
/// - CANCELED: any time an in-flight navigation is aborted (superseded / denied / error)
///
/// NOTE: Same-route requests should short-circuit silently (no events).
///
/// Event payload examples (JSON):
/// { "tx_id": 42, "to": "Home" }
/// { "tx_id": 42, "to": "Home", "allowed": true }
/// { "tx_id": 42, "reason": "superseded" }
///
/// MAYA DRY KISS: keep only externally meaningful milestones.
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
