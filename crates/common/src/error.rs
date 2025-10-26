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

    /// Errors originating from navigation orchestration (manager state / logic).
    #[error("{0}")]
    Navigation(#[from] NavigationError),

    /// Health / setup domain errors.
    #[error("{0}")]
    Health(#[from] HealthError),

    /// Intro engine domain errors.
    #[error("{0}")]
    Intro(#[from] IntroError),

    /// Instrument engine domain errors.
    #[error("{0}")]
    Instrument(#[from] InstrumentError),

    /// Tuner domain errors.
    #[error("{0}")]
    Tuner(#[from] TunerError),

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

// -------- Domain: Setup (window & initial app wiring) --------
/// Errors arising during early application setup (window acquisition, sizing,
/// appearance updates, event emission, etc.). This separates one‑time
/// initialization issues from longer‑lived health/runtime domains.
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

// -------- Domain: Navigation Manager --------
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

impl From<NavGateError> for AppError {
    fn from(value: NavGateError) -> Self {
        AppError::Navigation(value.into())
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
    #[error("instrument start failed")]
    StartFailed { detail: Option<String> },
    #[error("instrument control error: {0}")]
    Control(#[from] ControlError),
    #[error("device unavailable")]
    DeviceUnavailable,
    #[error("default output config unavailable")]
    OutputConfigUnavailable,
    #[error("default input config unavailable")]
    InputConfigUnavailable,
    #[error("unsupported sample format: {0}")]
    UnsupportedSampleFormat(String),
    #[error("stream build failed: {detail}")]
    BuildStream { detail: String },
    #[error("control acknowledgement timeout (op={op})")]
    AckTimeout { op: String },
    #[error("control thread join failed (op={op})")]
    ThreadJoin { op: String },
    #[error("backend missing (op={op})")]
    BackendMissing { op: String },
    #[error("instrument config error: {0}")]
    ConfigError(#[from] InstrumentConfigError),
    #[error("instrument not initialized")]
    NotInitialized,
}

#[derive(Debug, Error, Serialize, Deserialize)]
/// Errors controlling playback thread
pub enum ControlError {
    #[error("control channel send failed (op={op})")]
    ChannelSend { op: String },
    #[error("control acknowledgement timeout (op={op})")]
    AckTimeout { op: String },
    #[error("control thread join failed (op={op})")]
    ThreadJoin { op: String },
    #[error("backend missing (op={op})")]
    BackendMissing { op: String },
    #[error("stream build failed: {detail}")]
    BuildStream { detail: String },
    #[error("node not found: {key:?}")]
    NodeNotFound { key: crate::NodeKey },
}

impl From<ControlError> for AppError {
    fn from(value: ControlError) -> Self {
        AppError::Instrument(InstrumentError::Control(value))
    }
}

#[derive(Debug, Error, Serialize, Deserialize)]
pub enum InstrumentConfigError {
    #[error("node {node} frequency {freq} above recommended")]
    NodeFreqencyAboveRecomended { node: usize, freq: f32 },

    #[error("node {node} frequency {freq} below recommended")]
    NodeFreqencyBelowRecomended { node: usize, freq: f32 },

    #[error("node {node} frequency {freq} above safe")]
    NodeFreqencyAboveSafe { node: usize, freq: f32 },

    #[error("node {node} frequency {freq} below safe")]
    NodeFreqencyBelowSafe { node: usize, freq: f32 },

    #[error("node {node} band start {freq} above recommended")]
    NodeBandStartAboveRecomended { node: usize, freq: f32 },

    #[error("node {node} band start {freq} below recommended")]
    NodeBandStartBelowRecomended { node: usize, freq: f32 },

    #[error("node {node} band start {freq} above safe")]
    NodeBandStartAboveSafe { node: usize, freq: f32 },

    #[error("node {node} band start {freq} below safe")]
    NodeBandStartBelowSafe { node: usize, freq: f32 },

    #[error("node {node} band end {freq} above recommended")]
    NodeBandEndAboveRecomended { node: usize, freq: f32 },

    #[error("node {node} band end {freq} below recommended")]
    NodeBandEndBelowRecomended { node: usize, freq: f32 },

    #[error("node {node} band end {freq} above safe")]
    NodeBandEndAboveSafe { node: usize, freq: f32 },

    #[error("node {node} band end {freq} below safe")]
    NodeBandEndBelowSafe { node: usize, freq: f32 },

    #[error("max cumulative gain above safe: {0}")]
    MaxCumulativeGainAboveSafe(f32),

    #[error("channels configuration invalid")]
    ChannelsConfigurationInvalid,

    #[error("empty config")]
    Empty,

    #[error("some groups have varying number of nodes")]
    InvalidGroups,

    #[error("empty group")]
    EmptyGroup,
}

impl From<InstrumentConfigError> for AppError {
    fn from(value: InstrumentConfigError) -> Self {
        AppError::Instrument(value.into())
    }
}

/// Tuner-specific errors
#[derive(Debug, Error, Serialize, Deserialize)]
pub enum TunerError {
    #[error("invalid sensor index: {index}")]
    InvalidSensorIndex { index: usize },

    #[error("failed to emit event {event}: {message}")]
    Emit { event: String, message: String },

    #[error("spectrum data not available")]
    SpectrumDataUnavailable,

    #[error("FFT analysis failed: {message}")]
    FFTAnalysisFailed { message: String },

    #[error("invalid frequency range: {min_freq} to {max_freq}")]
    InvalidFrequencyRange { min_freq: f32, max_freq: f32 },

    #[error("invalid magnitude range: {min_mag} to {max_mag}")]
    InvalidMagnitudeRange { min_mag: f32, max_mag: f32 },
}

impl InstrumentConfigError {
    /// Returns true if the error indicates an unsafe configuration.
    pub fn is_unsafe(&self) -> bool {
        matches!(
            self,
            InstrumentConfigError::NodeFreqencyAboveSafe { .. }
                | InstrumentConfigError::NodeFreqencyBelowSafe { .. }
                | InstrumentConfigError::NodeBandStartAboveSafe { .. }
                | InstrumentConfigError::NodeBandStartBelowSafe { .. }
                | InstrumentConfigError::NodeBandEndAboveSafe { .. }
                | InstrumentConfigError::NodeBandEndBelowSafe { .. }
                | InstrumentConfigError::MaxCumulativeGainAboveSafe(_)
        )
    }
}

// -------- Feature-gated conversions --------
#[cfg(feature = "tauri")]
impl From<tauri::Error> for AppError {
    fn from(value: tauri::Error) -> Self {
        AppError::Tauri(value.to_string())
    }
}
