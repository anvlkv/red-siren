//! Web Audio Tuner Implementation
//!
//! PURPOSE
//! -------
//! Temporary stub implementation of TunerRuntime for the Web Audio migration.
//! This provides a null implementation that allows compilation while the full
//! Web Audio tuner integration is implemented in future phases.
//!
//! MAYA DRY KISS
//! -------------
//! - Simple null implementation without complex dependencies
//! - Clear placeholder for future Web Audio integration
//! - Minimal viable implementation to unblock migration

use common::audio::TunerRuntime;
use common::error::Result;
use common::tuner::{Config, SpectrumData};

/// Null tuner runtime implementation
pub struct WebTunerRuntime {
    _config: Option<Config>,
}

impl WebTunerRuntime {
    pub fn new() -> Self {
        Self { _config: None }
    }
}

impl TunerRuntime for WebTunerRuntime {
    fn start(&self, config: &Config) -> Result<()> {
        log::info!("Web tuner started (null implementation)");
        Ok(())
    }

    fn stop(&self) {
        log::info!("Web tuner stopped");
    }

    fn update_config(&self, _config: &Config) {
        log::info!("Web tuner config updated (null implementation)");
    }

    fn poll_spectrum(&self) -> Option<SpectrumData> {
        // Null implementation - no spectrum data available
        None
    }
}

/// Factory function to create web tuner runtime
pub fn make_web_tuner_runtime() -> Box<dyn TunerRuntime + Send + Sync> {
    Box::new(WebTunerRuntime::new())
}
