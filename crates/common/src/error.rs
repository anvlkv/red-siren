#![allow(clippy::module_name_repetitions)]
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
    /// Errors originating from app/window setup / initialization.
    #[error("{0}")]
    Setup(#[from] SetupError),

    /// Health / setup domain errors.
    #[error("{0}")]
    Health(#[from] HealthError),

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

#[derive(Debug, Error, Serialize, Deserialize)]
pub enum SetupError {
    #[error("main window not found")]
    MainWindowMissing,

    #[error("window query failed: {message}")]
    WindowQuery { message: String },

    #[error("window state operation failed (op={op}): {message}")]
    WindowStateOp { op: String, message: String },

    #[error("appearance update failed: {message}")]
    Appearance { message: String },

    #[error("window operation failed (op={op}): {message}")]
    WindowOp { op: String, message: String },

    #[error("emit failed (event={event}): {message}")]
    Emit { event: String, message: String },

    #[error("store setup error: {message}")]
    StoreSetupErr { message: String },
}

impl SetupError {
    pub fn window_query<E: ToString>(e: E) -> Self {
        SetupError::WindowQuery {
            message: e.to_string(),
        }
    }
    pub fn window_state_op<S: Into<String>, E: ToString>(op: S, e: E) -> Self {
        SetupError::WindowStateOp {
            op: op.into(),
            message: e.to_string(),
        }
    }
    pub fn appearance<E: ToString>(e: E) -> Self {
        SetupError::Appearance {
            message: e.to_string(),
        }
    }
    pub fn emit<S: Into<String>, E: ToString>(event: S, e: E) -> Self {
        SetupError::Emit {
            event: event.into(),
            message: e.to_string(),
        }
    }
}

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

// -------- Feature-gated conversions --------
#[cfg(feature = "tauri")]
impl From<tauri::Error> for AppError {
    fn from(value: tauri::Error) -> Self {
        AppError::Tauri(value.to_string())
    }
}
