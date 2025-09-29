//! Shared error definitions for the application.
//!
//! Design notes:
//! - Always serializable/deserializable.
//! - Domain‑specific enums kept small & focused.
//! - `Result<T>` alias centralizes the top‑level `AppError`.
//! - Only the `From<tauri::Error>` impl is feature‑gated (`tauri` feature).
//! - Domain -> AppError conversions are unconditional (cheap, harmless).

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Convenient application-wide result type.
pub type Result<T> = std::result::Result<T, AppError>;

/// Top-level application error envelope.
#[derive(Debug, Error, Serialize, Deserialize)]
// Consider a tagged representation if you want more explicit wire shape:
// #[serde(tag = "kind", content = "data")]
pub enum AppError {
    /// Errors originating from navigation orchestration (manager state / logic).
    #[error("{0}")]
    Navigation(#[from] NavigationError),

    /// Errors from navigation gating logic.
    #[error("{0}")]
    NavGate(#[from] NavGateError),

    /// Health / setup domain errors.
    #[error("{0}")]
    Health(#[from] HealthError),

    /// Intro engine domain errors.
    #[error("{0}")]
    Intro(#[from] IntroError),

    /// Instrument engine domain errors.
    #[error("{0}")]
    Instrument(#[from] InstrumentError),

    /// A generic internal error (catch‑all). Prefer more specific variants when reasonable.
    #[error("internal error: {message}")]
    Internal { message: String },

    /// Wrapped tauri error
    #[error("tauri error: {0}")]
    Tauri(String),
}

impl AppError {
    /// Helper to create an internal message error.
    pub fn internal<S: Into<String>>(msg: S) -> Self {
        AppError::Internal {
            message: msg.into(),
        }
    }
}

/// Navigation manager domain errors.
#[derive(Debug, Error, Serialize, Deserialize)]
pub enum NavigationError {
    #[error("navigation manager poisoned")]
    ManagerPoisoned,

    #[error("navigation state mismatch")]
    StateMismatch,

    #[error("navigation gating error: {0}")]
    GateError(#[from] NavGateError),
}

/// Errors produced by route gating logic.
#[derive(Debug, Error, Serialize, Deserialize)]
pub enum NavGateError {
    #[error("permission denied")]
    PermissionDenied,
    #[error("system busy")]
    Busy,
    #[error("unexpected: {0}")]
    Other(String),
}

impl NavGateError {
    /// Whether this is a *deny* style error (user / soft gating),
    /// as opposed to a hard internal failure.
    pub fn is_deny(&self) -> bool {
        matches!(self, NavGateError::PermissionDenied | NavGateError::Busy)
    }
}

// -------- Domain: Health --------
#[derive(Debug, Error, Serialize, Deserialize)]
pub enum HealthError {
    #[error("mic permission check failed")]
    MicPermissionCheckFailed { detail: Option<String> },
    #[error("emit failed (event={event}): {message}")]
    Emit { event: String, message: String },
    #[error("window operation failed (op={op}): {message}")]
    WindowOp { op: String, message: String },
    #[error("health state poisoned")]
    StatePoisoned,
}

// -------- Domain: Intro --------
#[derive(Debug, Error, Serialize, Deserialize)]
pub enum IntroError {
    #[error("intro engine state poisoned")]
    StatePoisoned,
    #[error("intro engine not ready")]
    EngineNotReady,
    #[error("pause failed")]
    PauseFailed { detail: Option<String> },
    #[error("resume failed")]
    ResumeFailed { detail: Option<String> },
    #[error("frame timestamp error")]
    FrameTimeError,
}

// -------- Domain: Instrument --------
#[derive(Debug, Error, Serialize, Deserialize)]
pub enum InstrumentError {
    #[error("emit failed (event={event}): {message}")]
    Emit { event: String, message: String },
    #[error("instrument state poisoned")]
    StatePoisoned,
    #[error("mic permission missing")]
    MicPermissionMissing,
    #[error("unsupported activation source: {0}")]
    UnsupportedActivationSource(u8),
    #[error("instrument pause failed")]
    PauseFailed { detail: Option<String> },
    #[error("instrument resume failed")]
    ResumeFailed { detail: Option<String> },
}

// -------- Feature-gated conversions --------

#[cfg(feature = "tauri")]
impl From<tauri::Error> for AppError {
    fn from(value: tauri::Error) -> Self {
        AppError::Tauri(value.to_string())
    }
}

// -------- Tests (minimal) --------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_nav_gate_error() {
        let err = AppError::from(NavGateError::PermissionDenied);
        let s = serde_json::to_string(&err).unwrap();
        assert!(s.contains("permission denied"));
    }
}
