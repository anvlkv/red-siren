/*!
Shared navigation event payloads.

WHY:
- Centralize event payload shapes used across backend (Tauri) and frontend (Leptos).
- Keep payloads typed with `RouteId` to avoid stringly-typed bugs.
- Own all string data so deserialization works reliably across boundaries.

NOTES:
- `NavCommittedPayload` includes both `to: RouteId` and the canonical `path` (String) to simplify client routing.
- `NavCanceledPayload.reason` is a short machine-readable string: "superseded" | "denied" | "error".

MAYA DRY KISS:
- Minimal types; only externally meaningful milestones are represented.
*/

use serde::{Deserialize, Serialize};

use crate::RouteId;

/// Emitted immediately when a navigation intent is received (before async gating).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavRequestedPayload {
    pub tx_id: u64,
    pub to: RouteId,
}

/// Emitted after gating completes (always), indicating if proceeding is allowed.
/// `to` is the intended destination (may be redirected later).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavGatedPayload {
    pub tx_id: u64,
    pub to: RouteId,
    pub allowed: bool,
}

/// Emitted when the transition starts after gating success.
/// `from` is the current route and `to` is the destination (may be redirected by policy).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavStartedPayload {
    pub tx_id: u64,
    pub from: RouteId,
    pub to: RouteId,
}

/// Emitted at the commit point (router path change).
/// Includes both `to` (typed) and the canonical `path` (string) for convenience.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavCommittedPayload {
    pub tx_id: u64,
    pub to: RouteId,
    pub path: String,
}

/// Emitted once the transition is fully settled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavCompletedPayload {
    pub tx_id: u64,
    pub to: RouteId,
}

/// Emitted on any interruption (superseded / denied / error).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavCanceledPayload {
    pub tx_id: u64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayloadTxTo {
    pub tx_id: u64,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayloadGated {
    pub tx_id: u64,
    pub to: String,
    pub allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayloadStarted {
    pub tx_id: u64,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayloadCommitted {
    pub tx_id: u64,
    pub to: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayloadCanceled {
    pub tx_id: u64,
    pub reason: String,
}
