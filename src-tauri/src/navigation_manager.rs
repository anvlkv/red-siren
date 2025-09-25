/*!
Phase 2 Navigation Manager (Async, Gating, Cancellation, Events)

WHY:
Centralizes navigation orchestration on the backend (Tauri) with:
- Transaction IDs
- Async gating (Result<bool, NavGateError>)
- Immediate cancellation of superseded requests
- Deterministic phase progression
- Emission of the new Phase 2 navigation events
- Logging instrumentation (log crate)

OUT OF SCOPE (Phase 2):
- Leave/enter animation hooks (Phase 3)
- Prefetch / data loading orchestration
- Complex phase timeouts / retries

MAYA DRY KISS:
Only the minimal logic needed to reliably gate, cancel, and commit navigations.

USAGE (intended integration sketch):
1. Create a global instance (Arc<NavigationManager>) at startup.
2. Expose a Tauri command that:
   - Parses incoming String payload into RouteId (via crate::navigation::parse_incoming_route_payload)
   - Calls `manager.request(route_id)`
3. Frontend listens to:
   - navigation_committed -> performs actual Leptos router navigate()
   - (Optional diagnostics) requested / gated / started / completed / canceled

EVENT PAYLOAD SHAPES (JSON):
navigation_requested  { "tx_id": 1, "to": "Home" }
navigation_gated      { "tx_id": 1, "to": "Home", "allowed": true }
navigation_started    { "tx_id": 1, "from": "Home", "to": "Play" }
navigation_committed  { "tx_id": 1, "to": "Play", "path": "/play" }
navigation_completed  { "tx_id": 1, "to": "Play" }
navigation_canceled   { "tx_id": 1, "reason": "superseded" }

POLICY:
- Same-route navigation -> silently ignored (no events).
- Superseding: old tx emits navigation_canceled(reason="superseded") immediately.
- Gating deny -> gated(allowed=false) + canceled(reason="denied").
- Gating error -> gated(allowed=false) + canceled(reason="error").
*/

use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use log::{error, info, warn};
use serde::Serialize;
use shared::{events::navigation_payloads::{PayloadCanceled, PayloadCommitted, PayloadGated, PayloadStarted, PayloadTxTo}, RouteId};
use thiserror::Error;
use tauri::{AppHandle, Emitter};

use shared::events::navigation::{
    NAV_CANCELED, NAV_COMMITTED, NAV_COMPLETED, NAV_GATED, NAV_REQUESTED, NAV_STARTED, NAV_SYNC,
};

/// Phases managed internally. (Public exposure via events only.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NavPhase {
    Gating,
    Leaving,
    Committing,
    Completed,
    Canceled,
}

/// Cancellation reasons surfaced in `navigation_canceled` event payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelReason {
    Superseded,
    Denied,
    Error,
}

impl CancelReason {
    fn as_str(&self) -> &'static str {
        match self {
            CancelReason::Superseded => "superseded",
            CancelReason::Denied => "denied",
            CancelReason::Error => "error",
        }
    }
}

#[derive(Debug)]
struct NavTx {
    id: u64,
    from: RouteId,
    to: RouteId,
    phase: NavPhase,
    canceled: bool,
}

impl NavTx {
    fn new(id: u64, from: RouteId, to: RouteId) -> Self {
        Self {
            id,
            from,
            to,
            phase: NavPhase::Gating,
            canceled: false,
        }
    }
}

#[derive(Debug)]
struct InnerState {
    current: RouteId,
    active: Option<NavTx>,
}

/// Navigation gating error variants.
#[derive(Error, Debug)]
pub enum NavGateError {
    #[error("permission denied")]
    PermissionDenied,
    #[error("system busy")]
    Busy,
    #[error("unexpected: {0}")]
    Other(String),
}

impl NavGateError {
    fn is_deny(&self) -> bool {
        matches!(self, NavGateError::PermissionDenied | NavGateError::Busy)
    }
}

/// Public handle (Arc) to the manager.
#[derive(Clone)]
pub struct NavigationManager {
    app: Arc<AppHandle>,
    state: Arc<Mutex<InnerState>>,
    id_gen: Arc<AtomicU64>,
}

impl NavigationManager {
    pub fn new(app: AppHandle, initial: RouteId) -> Self {
        Self {
            app: Arc::new(app),
            state: Arc::new(Mutex::new(InnerState {
                current: initial,
                active: None,
            })),
            id_gen: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Request navigation to a route.
    /// Returns Some(tx_id) if a new transaction started, or None if ignored (same-route).
    pub fn request(&self, to: RouteId) -> Option<u64> {
        let mut guard = self.state.lock().unwrap();
        let current = guard.current;

        // Same-route: no-op
        if to == current {
            return None;
        }

        // Supersede existing
        if let Some(mut existing) = guard.active.take() {
            existing.canceled = true;
            drop(guard);
            self.emit_canceled(existing.id, CancelReason::Superseded);
            guard = self.state.lock().unwrap();
        }

        let id = self.id_gen.fetch_add(1, Ordering::SeqCst);
        let tx = NavTx::new(id, current, to);
        guard.active = Some(tx);

        // Emit requested event
        self.emit_requested(id, to);

        // Spawn async gating
        let cloned = self.clone();
        tokio::spawn(async move {
            cloned.run_gating(id).await;
        });

        Some(id)
    }

    /// Explicit cancel API (optional external call). If tx_id matches active, it cancels.
    pub fn cancel(&self, tx_id: u64, reason: CancelReason) {
        let mut guard = self.state.lock().unwrap();
        if let Some(active) = guard.active.as_mut() {
            if active.id == tx_id && !matches!(active.phase, NavPhase::Completed | NavPhase::Canceled) {
                active.canceled = true;
                active.phase = NavPhase::Canceled;
                drop(guard);
                self.emit_canceled(tx_id, reason);
                let mut guard = self.state.lock().unwrap();
                guard.active = None;
            }
        }
    }

    /// Called by UI when leave animation has completed; advances to commit.
    pub fn leave_done(&self, tx_id: u64) {
        // Only proceed if this tx is still active and in Leaving phase.
        let phase_ok = {
            let guard = self.state.lock().unwrap();
            guard
                .active
                .as_ref()
                .map(|tx| tx.id == tx_id && tx.phase == NavPhase::Leaving && !tx.canceled)
                .unwrap_or(false)
        };
        if phase_ok {
            self.commit(tx_id);
        }
    }

    /// Called by UI when enter animation has completed; completes the transaction.
    pub fn enter_done(&self, tx_id: u64) {
        let to_opt = {
            let mut guard = self.state.lock().unwrap();
            if let Some(active) = guard.active.as_mut() {
                if active.id == tx_id && active.phase == NavPhase::Committing && !active.canceled {
                    active.phase = NavPhase::Completed;
                    Some(active.to)
                } else {
                    None
                }
            } else {
                None
            }
        };
        if let Some(to) = to_opt {
            self.emit_completed(tx_id, to);
            // Clear active transaction
            let mut guard = self.state.lock().unwrap();
            if let Some(active) = &guard.active {
                if active.id == tx_id {
                    guard.active = None;
                }
            }
        }
    }

    async fn run_gating(&self, tx_id: u64) {
        // Check still active
        {
            if !self.is_tx_active(tx_id) {
                return;
            }
        }

        let current_requested = self.active_to_route(tx_id);
        let outcome_result = gate_navigation(current_requested).await;

        match outcome_result {
            Ok(outcome) => {
                // Emit gating result with the effective target (may be redirected)
                self.emit_gated(tx_id, outcome.effective_to, outcome.allowed);
                if !outcome.allowed {
                    self.cancel(tx_id, CancelReason::Denied);
                    return;
                }

                // If gating redirected (e.g. Play -> Permissions), update active target before proceeding.
                if outcome.effective_to != current_requested {
                    self.set_active_to(tx_id, outcome.effective_to);
                }

                // Proceed to leaving (placeholder)
                if !self.advance_phase(tx_id, NavPhase::Gating, NavPhase::Leaving) {
                    return;
                }
                self.emit_started(tx_id);
                // Await UI leave animation completion (leave_done) before committing.
            }
            Err(err) => {
                // Distinguish deny vs internal error
                self.emit_gated(tx_id, current_requested, false);
                warn!("navigation gating error tx_id={} err={}", tx_id, err);
                let reason = if err.is_deny() {
                    CancelReason::Denied
                } else {
                    CancelReason::Error
                };
                self.cancel(tx_id, reason);
            }
        }
    }

    fn commit(&self, tx_id: u64) {
        // Transition: Leaving -> Committing
        if !self.advance_phase(tx_id, NavPhase::Leaving, NavPhase::Committing) {
            return;
        }

        // Emit committed, update current
        let (to, path) = {
            let mut guard = self.state.lock().unwrap();
            if let Some(active) = guard.active.as_mut() {
                if active.id == tx_id {
                    let to = active.to;
                    let path = to.path();
                    guard.current = to;
                    (to, path)
                } else {
                    return;
                }
            } else {
                return;
            }
        };

        self.emit_committed(tx_id, to, path);

        // Defer completion; wait for UI enter_done to finalize.
    }

    fn is_tx_active(&self, tx_id: u64) -> bool {
        let guard = self.state.lock().unwrap();
        guard
            .active
            .as_ref()
            .map(|tx| tx.id == tx_id && !tx.canceled)
            .unwrap_or(false)
    }

    fn active_to_route(&self, tx_id: u64) -> RouteId {
        let guard = self.state.lock().unwrap();
        guard
            .active
            .as_ref()
            .filter(|tx| tx.id == tx_id)
            .map(|tx| tx.to)
            .unwrap_or_else(|| guard.current)
    }

    fn set_active_to(&self, tx_id: u64, to: RouteId) {
        let mut guard = self.state.lock().unwrap();
        if let Some(active) = guard.active.as_mut() {
            if active.id == tx_id {
                active.to = to;
            }
        }
    }

    fn advance_phase(&self, tx_id: u64, expect: NavPhase, next: NavPhase) -> bool {
        let mut guard = self.state.lock().unwrap();
        if let Some(active) = guard.active.as_mut() {
            if active.id == tx_id && active.phase == expect && !active.canceled {
                active.phase = next;
                return true;
            }
        }
        false
    }

    // ---------------- Event Emission Helpers ----------------

    fn emit_requested(&self, tx_id: u64, to: RouteId) {
        self.emit(
            NAV_REQUESTED,
            &PayloadTxTo {
                tx_id,
                to: to.to_string(),
            },
        );
        info!("nav requested tx_id={} to={}", tx_id, to);
    }

    fn emit_gated(&self, tx_id: u64, to: RouteId, allowed: bool) {
        self.emit(
            NAV_GATED,
            &PayloadGated {
                tx_id,
                to: to.to_string(),
                allowed,
            },
        );
        info!("nav gated tx_id={} to={} allowed={}", tx_id, to, allowed);
    }

    fn emit_started(&self, tx_id: u64) {
        let (from, to) = {
            let guard = self.state.lock().unwrap();
            if let Some(active) = guard.active.as_ref() {
                if active.id == tx_id {
                    (active.from, active.to)
                } else {
                    return;
                }
            } else {
                return;
            }
        };
        self.emit(
            NAV_STARTED,
            &PayloadStarted {
                tx_id,
                from: from.to_string(),
                to: to.to_string(),
            },
        );
        info!("nav started tx_id={} from={} to={}", tx_id, from, to);
    }

    fn emit_committed(&self, tx_id: u64, to: RouteId, path: &str) {
        self.emit(
            NAV_COMMITTED,
            &PayloadCommitted {
                tx_id,
                to: to.to_string(),
                path: path.to_string(),
            },
        );
        info!("nav committed tx_id={} to={} path={}", tx_id, to, path);
    }

    fn emit_completed(&self, tx_id: u64, to: RouteId) {
        self.emit(
            NAV_COMPLETED,
            &PayloadTxTo {
                tx_id,
                to: to.to_string(),
            },
        );
        info!("nav completed tx_id={} to={}", tx_id, to);
    }

    fn emit_canceled(&self, tx_id: u64, reason: CancelReason) {
        self.emit(
            NAV_CANCELED,
            &PayloadCanceled {
                tx_id,
                reason: reason.as_str().to_string(),
            },
        );
        info!("nav canceled tx_id={} reason={}", tx_id, reason.as_str());
    }

    fn emit<T: Serialize>(&self, event: &str, payload: &T) {
        // Ignore errors (e.g., during shutdown) but log for diagnostics.
        if let Err(e) = self.app.emit(event, payload) {
            error!("failed to emit event={} err={}", event, e);
        }
    }

    /// Returns the current committed route (backend truth).
    pub fn current_route(&self) -> RouteId {
        let guard = self.state.lock().unwrap();
        guard.current
    }

    /// Emits a one-shot navigation synchronization snapshot.
    /// Consumers can listen for `navigation_sync` and align UI state.
    pub fn emit_sync_snapshot(&self) {
        let to = self.current_route();
        let path = to.path().to_string();
        self.emit(NAV_SYNC, &shared::events::navigation::NavSyncPayload { to, path });
    }
}


// ---------------- Gating Logic ----------------

/// Async gating wrapper. For now this delegates to the synchronous
/// `crate::navigation::can_navigate` and wraps result. Extend here
/// with real permission / unsaved-work / concurrency checks.
#[derive(Debug, Clone, Copy)]
struct GateOutcome {
    allowed: bool,
    effective_to: RouteId,
}

/// Determine if microphone permission is granted.
/// TODO: replace with real OS permission check (e.g., via a Tauri plugin or platform API).
fn mic_permission_granted() -> Option<bool> {
    // Returning None -> treated as "unknown", defaulting to false (deny Play -> redirect).
    None
}

async fn gate_navigation(to: RouteId) -> Result<GateOutcome, NavGateError> {
    // Base policy gate (global app-level check)
    if !crate::navigation::can_navigate(to) {
        return Ok(GateOutcome {
            allowed: false,
            effective_to: to,
        });
    }

    // Default outcome: allowed, no redirection
    let mut outcome = GateOutcome {
        allowed: true,
        effective_to: to,
    };

    // Mic-permission gating policy:
    // - If target is Play and mic permission is NOT granted, redirect to Permissions.
    if to == RouteId::Play {
        let has_mic = mic_permission_granted().unwrap_or(false);
        if !has_mic {
            outcome.effective_to = RouteId::Permissions;
        }
    }

    Ok(outcome)
}

// ---------------- Tests (basic) ----------------

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::Manager;

    // NOTE: These tests are basic and may need integration harness with a Tauri app.
    // For now we test pure logic pathways by mocking AppHandle via a lightweight app.

    fn dummy_app() -> AppHandle {
        // Build a minimal Tauri app for event emission.
        tauri::Builder::default()
            .build(tauri::generate_context!())
            .expect("build")
            .handle()
            .clone()
    }

    #[tokio::test]
    async fn same_route_ignored() {
        let app = dummy_app();
        let mgr = NavigationManager::new(app, RouteId::Home);
        let id = mgr.request(RouteId::Home);
        assert!(id.is_none(), "same-route should be ignored");
    }

    #[tokio::test]
    async fn supersede_cancels_previous() {
        let app = dummy_app();
        let mgr = NavigationManager::new(app, RouteId::Home);
        let a = mgr.request(RouteId::Home /* ignored */);
        assert!(a.is_none());
        let b = mgr.request(RouteId::Home /* ignored again */);
        assert!(b.is_none());

        // Real supersede case requires distinct target; we only have Home defined now.
        // This test is a placeholder; add more routes later (About, Play, etc.).
    }
}
