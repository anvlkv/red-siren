use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

use log::{error, info, warn};
use serde::Serialize;
use shared::{
    events::navigation_payloads::{
        NavCanceledPayload, NavCommittedPayload, NavCompletedPayload, NavGatedPayload,
        NavStartedPayload,
    },
    RouteId,
};
use tauri::{async_runtime::spawn, AppHandle, Emitter};

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
    pending: Option<RouteId>,
    /// Stack of previously visited (committed) routes for back navigation.
    history: Vec<RouteId>,
    /// When true, the next navigation commit will NOT push the current route
    /// onto history (used for back navigations to prevent forward-looping).
    suppress_history_push: bool,
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
                pending: None,
                history: Vec::new(),
                suppress_history_push: false,
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
        spawn(async move {
            cloned.run_gating(id).await;
        });

        Some(id)
    }

    /// Explicit cancel API (optional external call). If tx_id matches active, it cancels.
    pub fn cancel(&self, tx_id: u64, reason: CancelReason) {
        let mut guard = self.state.lock().unwrap();
        if let Some(active) = guard.active.as_mut() {
            if active.id == tx_id
                && !matches!(active.phase, NavPhase::Completed | NavPhase::Canceled)
            {
                active.canceled = true;
                active.phase = NavPhase::Canceled;
                drop(guard);
                self.emit_canceled(tx_id, reason);
                let mut guard = self.state.lock().unwrap();
                guard.active = None;
            }
        }
    }

    /// Initiate a back navigation if possible.
    /// Returns Some(tx_id) of the new navigation transaction, or None if history empty.
    pub fn back(&self) -> Option<u64> {
        let target = {
            let mut guard = self.state.lock().unwrap();
            if let Some(route) = guard.history.pop() {
                // Suppress history push for this backward nav to avoid looping.
                guard.suppress_history_push = true;
                route
            } else {
                return None;
            }
        };
        self.request(target)
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

    pub fn resume_pending(&self) -> Option<u64> {
       self.take_pending().and_then(|to| self.request(to))
    }

    /// Bootstrap the initial route as a full navigation transaction (tx_id=0).
    /// Emits requested -> gated -> started -> committed (from==to) and leaves the tx
    /// in Committing phase so UI can finish it with enter_done (after appear animation).
    /// Idempotent: if any tx already started (id_gen advanced) or an active tx id=0 exists,
    /// it short-circuits and returns 0.
    pub fn bootstrap_initial_transaction(&self) -> u64 {
        // Detect prior bootstrap or progressed id space.
        {
            let guard = self.state.lock().unwrap();
            if self.id_gen.load(Ordering::SeqCst) > 1 {
                if let Some(active) = &guard.active {
                    if active.id == 0 {
                        return 0;
                    }
                }
                return 0;
            }
        }

        let current = self.current_route();
        let tx_id = 0;

        {
            let mut guard = self.state.lock().unwrap();
            guard.active = Some(NavTx::new(tx_id, current, current));
        }

        // Synchronous lifecycle (no async gating for bootstrap)
        self.emit_requested(tx_id, current);
        self.emit_gated(tx_id, current, true);

        // Advance to Leaving (mirrors post-gating phase)
        {
            let mut guard = self.state.lock().unwrap();
            if let Some(active) = guard.active.as_mut() {
                if active.id == tx_id {
                    active.phase = NavPhase::Leaving;
                }
            }
        }

        // NOT EMITTING started as there's no page to leave...
        // self.emit_started(tx_id);

        // Immediate commit (no actual "leave" animation for an initial page)
        self.commit(tx_id);

        info!(
            "bootstrap initial navigation established tx_id={} route={}",
            tx_id, current
        );

        tx_id
    }

    async fn run_gating(&self, tx_id: u64) {
        // Check still active
        {
            if !self.is_tx_active(tx_id) {
                return;
            }
        }

        let current_requested = self.active_to_route(tx_id);
        let outcome_result = super::gates::gate_navigation(current_requested, &self.app).await;

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
                    // store pending for potential resume
                    self.set_pending(current_requested);
                    // update active navigation target
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
        let to = {
            let mut guard = self.state.lock().unwrap();
            if let Some((from, to, id)) = guard
                .active
                .as_ref()
                .filter(|a| a.id == tx_id)
                .map(|a| (a.from, a.to, a.id))
            {
                // Push prior route onto history only for successful forward navigations:
                // - Not suppressed (i.e., not a back navigation)
                // - Different route
                // - Not the bootstrap (tx_id != 0)
                if !guard.suppress_history_push && from != to && id != 0 {
                    guard.history.push(from);
                }
                // Always clear suppression flag after a commit cycle.
                guard.suppress_history_push = false;
                guard.current = to;
                to
            } else {
                return;
            }
        };

        self.emit_committed(tx_id, to);

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

    fn set_pending(&self, to: RouteId) {
        let mut guard = self.state.lock().unwrap();
        guard.pending = Some(to);
    }

    fn take_pending(&self) -> Option<RouteId> {
        let mut guard = self.state.lock().unwrap();
        guard.pending.take()
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
        self.emit(NAV_REQUESTED, &NavCompletedPayload { tx_id, to });
        info!("nav requested tx_id={} to={}", tx_id, to);
    }

    fn emit_gated(&self, tx_id: u64, to: RouteId, allowed: bool) {
        self.emit(NAV_GATED, &NavGatedPayload { tx_id, to, allowed });
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
        self.emit(NAV_STARTED, &NavStartedPayload { tx_id, from, to });
        info!("nav started tx_id={} from={} to={}", tx_id, from, to);
    }

    fn emit_committed(&self, tx_id: u64, to: RouteId) {
        self.emit(NAV_COMMITTED, &NavCommittedPayload { tx_id, to });
        info!("nav committed tx_id={tx_id} to={to}");
    }

    fn emit_completed(&self, tx_id: u64, to: RouteId) {
        self.emit(NAV_COMPLETED, &NavCompletedPayload { tx_id, to });
        info!("nav completed tx_id={} to={}", tx_id, to);
    }

    fn emit_canceled(&self, tx_id: u64, reason: CancelReason) {
        self.emit(
            NAV_CANCELED,
            &NavCanceledPayload {
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
        self.emit(NAV_SYNC, &shared::events::navigation::NavSyncPayload { to });
    }
}
