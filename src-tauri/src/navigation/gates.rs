use shared::RouteId;
use tauri::{AppHandle, Manager};
use thiserror::Error;


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
    pub fn is_deny(&self) -> bool {
        matches!(self, NavGateError::PermissionDenied | NavGateError::Busy)
    }
}

/// Async gating wrapper. For now this delegates to the synchronous
/// `crate::navigation::can_navigate` and wraps result. Extend here
/// with real permission / unsaved-work / concurrency checks.
#[derive(Debug, Clone, Copy)]
pub struct GateOutcome {
    pub allowed: bool,
    pub effective_to: RouteId,
}

/// Backend gating policy stub.
/// Return `true` to allow, `false` to soft-deny (emits gated allowed=false + canceled),
/// or evolve into richer logic using external state. For *hard* internal errors,
/// adjust the manager's gating wrapper (it currently treats non-deny errors distinctly).
pub fn can_navigate(_to: RouteId) -> bool {
    true
}

pub async fn gate_navigation(to: RouteId, app: &AppHandle) -> Result<GateOutcome, NavGateError> {
    // Base policy gate (global app-level check)
    if !can_navigate(to) {
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

    let health_state = app.state::<crate::health::HealthSetupState>();

    let hs_guard = health_state.lock().await;

    match to {
        RouteId::Play => {
            if hs_guard.mic_permission.is_none() {
                outcome.effective_to = RouteId::Permissions;
            }
        }
        RouteId::Tune => {
            if hs_guard.mic_permission != Some(true) {
                outcome.effective_to = RouteId::Permissions;
            }
        }
        _=> {}
    }

    // // Mic-permission gating policy:
    // // - If target is Play and mic permission is NOT granted, redirect to Permissions.
    // if to == RouteId::Play {
    //     let has_mic = mic_permission_granted().unwrap_or(false);
    //     if !has_mic {
    //         outcome.effective_to = RouteId::Permissions;
    //     }
    // }

    Ok(outcome)
}
