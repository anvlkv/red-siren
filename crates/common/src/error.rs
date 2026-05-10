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
