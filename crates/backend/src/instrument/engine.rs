//! Instrument engine (runtime–agnostic).
//!
//! PURPOSE
//! -------
//! Maintains UI/state (layout, config, playback state, activation source)
//! and delegates actual audio I/O + DSP execution to the runtime abstraction
//! provided by `audio_system::rt`.
//!
//! RUNTIME SEPARATION
//! ------------------
//! The concrete audio backend (CPAL today, Web / Null later) lives in the
//! `audio-system` crate. This file never touches CPAL-specific types; it only
//! talks to the `StreamController` trait provided by `audio_system::rt`.
//!
//! FEATURE FLAGS
//! -------------
//! Runtime selection (CPAL, web, or null) is handled internally by `audio_system::rt`.
//!
//! DESIGN (MAYA DRY KISS)
//! ----------------------
//! - Minimal surface: only what the backend UI/commands require.
//! - No legacy aliases (old `cpal_audio` removed).
//! - Narrow lock scopes; derive config outside of second lock acquisitions.
//! - Avoid over‑engineering future runtimes; a Null controller already exists
//!   upstream, we just gate the usage here.
//!
//! THREADING
//! ---------
//! All state here uses `parking_lot::RwLock` for cheap synchronous access.
//! The heavy audio threads are owned by the runtime implementation.

use parking_lot::RwLock;

use audio_system::rt::{make_stream_controller, ActivationSource, StreamController};

use common::instrument::{Config as InstrumentConfig, Layout as InstrumentLayout};
use mint::Vector2;

/// Public wrapper so higher layers (commands) only hold one handle.
#[derive(Default)]
pub struct InstrumentEngine {
    pub(super) inner: Inner,
}

/// Internal engine state.
pub(super) struct Inner {
    // Playback & instrument state
    playing: RwLock<bool>,
    activation_source: RwLock<ActivationSource>,
    layout: RwLock<InstrumentLayout>,
    config: RwLock<InstrumentConfig>,

    // Runtime stream controller (lazy; concrete backend chosen by audio_system::rt)
    stream_controller: RwLock<Option<Box<dyn StreamController + Send + Sync>>>,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            playing: RwLock::new(false),
            activation_source: RwLock::new(ActivationSource::default()),
            layout: RwLock::new(InstrumentLayout::default()),
            config: RwLock::new(InstrumentConfig::default()),
            stream_controller: RwLock::new(None),
        }
    }
}

impl Inner {
    // ---------------------------------------------------------------------
    // Accessors
    // ---------------------------------------------------------------------

    pub fn layout(&self) -> InstrumentLayout {
        *self.layout.read()
    }

    pub fn playing(&self) -> bool {
        *self.playing.read()
    }

    pub fn activation_source(&self) -> ActivationSource {
        *self.activation_source.read()
    }

    // ---------------------------------------------------------------------
    // Playback lifecycle
    // ---------------------------------------------------------------------

    pub fn start_playback(&self) -> common::error::Result<bool> {
        if *self.playing.read() {
            return Ok(false);
        }

        {
            let mut p = self.playing.write();
            if *p {
                return Ok(false);
            }
            *p = true;
        }

        {
            let mut controller = self.stream_controller.write();
            if controller.is_none() {
                *controller = Some(make_stream_controller()?);
            }
            if let Some(ctrl) = controller.as_ref() {
                let layout = self.layout.read();
                let config = self.config.read();
                let source = *self.activation_source.read();
                ctrl.start(&layout, &config, source)?;
            }
        }

        Ok(true)
    }

    pub fn stop_playback(&self) -> common::error::Result<bool> {
        if !*self.playing.read() {
            return Ok(false);
        }

        if let Some(ctrl) = self.stream_controller.read().as_ref() {
            // Even if stop errors, proceed to mark stopped for consistency
            let _ = ctrl.stop();
        }

        {
            let mut p = self.playing.write();
            if !*p {
                return Ok(false);
            }
            *p = false;
        }

        Ok(true)
    }

    pub fn pause_playback(&self) -> common::error::Result<bool> {
        if !*self.playing.read() {
            return Ok(false);
        }

        if let Some(ctrl) = self.stream_controller.read().as_ref() {
            ctrl.pause()?;
        }

        {
            let mut p = self.playing.write();
            if !*p {
                return Ok(false);
            }
            *p = false;
        }

        Ok(true)
    }

    pub fn resume_playback(&self) -> common::error::Result<bool> {
        if *self.playing.read() {
            return Ok(false);
        }

        if let Some(ctrl) = self.stream_controller.read().as_ref() {
            ctrl.resume()?;
        }

        {
            let mut p = self.playing.write();
            if *p {
                return Ok(false);
            }
            *p = true;
        }

        Ok(true)
    }

    // ---------------------------------------------------------------------
    // Activation source
    // ---------------------------------------------------------------------

    pub fn set_activation_source(&self, src: ActivationSource) -> common::error::Result<bool> {
        let changed = {
            let mut current = self.activation_source.write();
            if *current != src {
                *current = src;
                true
            } else {
                false
            }
        };

        if changed {
            if let Some(ctrl) = self.stream_controller.read().as_ref() {
                ctrl.on_activation_source_changed(src)?;
            }
        }

        Ok(changed)
    }

    // ---------------------------------------------------------------------
    // Layout / Config maintenance
    // ---------------------------------------------------------------------

    pub fn set_is_dark(&self, is_dark: bool) -> common::error::Result<()> {
        // Derive new config with minimal lock hold time
        let new_cfg = {
            let mut layout = self.layout.write();
            layout.scale = if is_dark {
                common::instrument::Scale::In
            } else {
                common::instrument::Scale::Yo
            };
            InstrumentConfig::try_from(*layout)?
        };
        {
            let mut cfg = self.config.write();
            *cfg = new_cfg;
        }

        if let Some(ctrl) = self.stream_controller.read().as_ref() {
            let layout = self.layout.read();
            let config = self.config.read();
            ctrl.on_layout_changed(&layout, &config)?;
        }

        Ok(())
    }

    pub fn set_size(&self, width: f64, height: f64) -> common::error::Result<()> {
        let new_cfg = {
            let mut layout = self.layout.write();
            let scale = layout.scale;
            *layout = InstrumentLayout {
                scale,
                ..InstrumentLayout::from_screen_estate(Vector2 {
                    x: width as f32,
                    y: height as f32,
                })
            };
            InstrumentConfig::try_from(*layout)?
        };
        {
            let mut cfg = self.config.write();
            *cfg = new_cfg;
        }

        if let Some(ctrl) = self.stream_controller.read().as_ref() {
            let layout = self.layout.read();
            let config = self.config.read();
            ctrl.on_layout_changed(&layout, &config)?;
        }

        Ok(())
    }

    pub fn set_safe_area(
        &self,
        top: f32,
        right: f32,
        bottom: f32,
        left: f32,
    ) -> common::error::Result<()> {
        let new_cfg = {
            let mut layout = self.layout.write();
            let space = layout.space;
            let scale = layout.scale;
            *layout = InstrumentLayout::from_screen_estate_with_safe_area(
                space, top, right, bottom, left,
            );
            layout.scale = scale;
            InstrumentConfig::try_from(*layout)?
        };
        {
            let mut cfg = self.config.write();
            *cfg = new_cfg;
        }

        if let Some(ctrl) = self.stream_controller.read().as_ref() {
            let layout = self.layout.read();
            let config = self.config.read();
            ctrl.on_layout_changed(&layout, &config)?;
        }

        Ok(())
    }

    // ---------------------------------------------------------------------
    // Data taps
    // ---------------------------------------------------------------------

    pub fn snapshot_output_snoop(&self, group: usize, key: usize) -> Vec<f32> {
        if let Some(ctrl) = self.stream_controller.read().as_ref() {
            return ctrl.snapshot_output_snoop(group, key);
        }
        Vec::new()
    }

    pub fn snapshot_all_output_snoops(&self) -> Vec<(u8, u8, Vec<f32>)> {
        if let Some(ctrl) = self.stream_controller.read().as_ref() {
            return ctrl.snapshot_all_output_snoops();
        }
        Vec::new()
    }

    pub fn snapshot_activation_snoop(&self, group: usize, key: usize) -> Vec<f32> {
        if let Some(ctrl) = self.stream_controller.read().as_ref() {
            return ctrl.snapshot_activation_snoop(group, key);
        }
        Vec::new()
    }

    pub fn snapshot_all_activation_snoops(&self) -> Vec<(u8, u8, Vec<f32>)> {
        if let Some(ctrl) = self.stream_controller.read().as_ref() {
            return ctrl.snapshot_all_activation_snoops();
        }
        Vec::new()
    }
}
