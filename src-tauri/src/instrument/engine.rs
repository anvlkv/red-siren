use parking_lot::RwLock;

use common::instrument::{Config as InstrumentConfig, Layout as InstrumentLayout};
use mint::Vector2;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationSource {
    #[default]
    Entropy,
    Mic,
}

impl From<u8> for ActivationSource {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Entropy,
            _ => Self::Mic,
        }
    }
}

impl From<ActivationSource> for u8 {
    fn from(value: ActivationSource) -> u8 {
        match value {
            ActivationSource::Entropy => 0,
            ActivationSource::Mic => 1,
        }
    }
}

#[derive(Default)]
pub struct InstrumentEngine {
    pub(super) inner: Inner,
}

/// Common engine state and logic. This struct exists in all builds.
/// - In non-CPAL builds it maintains UI-facing state and derives configs.
/// - In CPAL builds it also hosts a stream controller that performs audio I/O.
pub(super) struct Inner {
    // UI/state shared across builds
    playing: RwLock<bool>,
    activation_source: RwLock<ActivationSource>,
    layout: RwLock<InstrumentLayout>,
    config: RwLock<InstrumentConfig>,

    // CPAL-specific stream controller (created on-demand)
    #[cfg(feature = "cpal_audio")]
    stream_controller: RwLock<Option<Box<dyn StreamController + Send + Sync>>>,
}

// Stream controller interface (only compiled with CPAL feature)
#[cfg(feature = "cpal_audio")]
pub trait StreamController {
    // Lifecycle
    fn start(
        &self,
        layout: &InstrumentLayout,
        config: &InstrumentConfig,
        source: ActivationSource,
    ) -> common::error::Result<()>;
    fn stop(&self) -> common::error::Result<()>;
    fn pause(&self) -> common::error::Result<()>;
    fn resume(&self) -> common::error::Result<()>;

    // Reactivity
    fn on_activation_source_changed(&self, source: ActivationSource) -> common::error::Result<()>;
    fn on_layout_changed(
        &self,
        layout: &InstrumentLayout,
        config: &InstrumentConfig,
    ) -> common::error::Result<()>;

    // Data taps
    fn snapshot_output_snoop(&self, group: usize, key: usize) -> Vec<f32>;
    fn snapshot_all_output_snoops(&self) -> Vec<(u8, u8, Vec<f32>)>;
}

// Factory for CPAL controller (provided by engine_cpal.rs)
#[cfg(feature = "cpal_audio")]
fn make_stream_controller() -> common::error::Result<Box<dyn StreamController + Send + Sync>> {
    // Delegated to the CPAL backend module
    crate::instrument::make_stream_controller()
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            playing: RwLock::new(false),
            activation_source: RwLock::new(ActivationSource::default()),
            layout: RwLock::new(InstrumentLayout::default()),
            config: RwLock::new(InstrumentConfig::default()),
            #[cfg(feature = "cpal_audio")]
            stream_controller: RwLock::new(None),
        }
    }
}

impl Inner {
    // Snapshot current layout
    pub fn layout(&self) -> InstrumentLayout {
        *self.layout.read()
    }

    // Playback control
    pub fn start_playback(&self) -> common::error::Result<bool> {
        if *self.playing.read() {
            return Ok(false);
        }

        // Set playing first to avoid races in callers
        {
            let mut p = self.playing.write();
            if *p {
                return Ok(false);
            }
            *p = true;
        }

        // CPAL: start controller with current state
        #[cfg(feature = "cpal_audio")]
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

        // CPAL: stop controller first
        #[cfg(feature = "cpal_audio")]
        {
            if let Some(ctrl) = self.stream_controller.read().as_ref() {
                // If stop fails, still mark as stopped for consistency
                let _ = ctrl.stop();
            }
        }

        // Mark stopped
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

        // CPAL: pause controller
        #[cfg(feature = "cpal_audio")]
        {
            if let Some(ctrl) = self.stream_controller.read().as_ref() {
                ctrl.pause()?;
            }
        }

        // Mark not playing
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

        // CPAL: resume controller
        #[cfg(feature = "cpal_audio")]
        {
            if let Some(ctrl) = self.stream_controller.read().as_ref() {
                ctrl.resume()?;
            }
        }

        // Mark playing
        {
            let mut p = self.playing.write();
            if *p {
                return Ok(false);
            }
            *p = true;
        }

        Ok(true)
    }

    pub fn playing(&self) -> bool {
        *self.playing.read()
    }

    // Activation source
    pub fn activation_source(&self) -> ActivationSource {
        *self.activation_source.read()
    }

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

        #[cfg(feature = "cpal_audio")]
        if changed {
            if let Some(ctrl) = self.stream_controller.read().as_ref() {
                ctrl.on_activation_source_changed(src)?;
            }
        }

        Ok(changed)
    }

    // Layout/config maintenance

    pub fn set_is_dark(&self, is_dark: bool) -> common::error::Result<()> {
        // Narrow lock scope; derive outside second lock
        let new_cfg = {
            let mut layout = self.layout.write();
            layout.scale = if is_dark {
                common::instrument::Scale::In
            } else {
                common::instrument::Scale::Yo
            };
            common::instrument::Config::try_from(*layout)?
        };
        {
            let mut cfg = self.config.write();
            *cfg = new_cfg;
        }

        #[cfg(feature = "cpal_audio")]
        if let Some(ctrl) = self.stream_controller.read().as_ref() {
            let layout = self.layout.read();
            let config = self.config.read();
            ctrl.on_layout_changed(&layout, &config)?;
        }

        Ok(())
    }

    pub fn set_size(&self, width: f64, height: f64) -> common::error::Result<()> {
        // Update layout and derive config
        let new_cfg = {
            let mut layout = self.layout.write();
            let scale = layout.scale;
            *layout = common::instrument::Layout {
                scale,
                ..common::instrument::Layout::from_screen_estate(Vector2 {
                    x: width as f32,
                    y: height as f32,
                })
            };
            common::instrument::Config::try_from(*layout)?
        };
        {
            let mut cfg = self.config.write();
            *cfg = new_cfg;
        }

        #[cfg(feature = "cpal_audio")]
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
            *layout = common::instrument::Layout::from_screen_estate_with_safe_area(
                space, top, right, bottom, left,
            );
            layout.scale = scale;
            common::instrument::Config::try_from(*layout)?
        };
        {
            let mut cfg = self.config.write();
            *cfg = new_cfg;
        }

        #[cfg(feature = "cpal_audio")]
        if let Some(ctrl) = self.stream_controller.read().as_ref() {
            let layout = self.layout.read();
            let config = self.config.read();
            ctrl.on_layout_changed(&layout, &config)?;
        }

        Ok(())
    }

    // Data taps

    pub fn snapshot_output_snoop(&self, group: usize, key: usize) -> Vec<f32> {
        #[cfg(feature = "cpal_audio")]
        {
            if let Some(ctrl) = self.stream_controller.read().as_ref() {
                return ctrl.snapshot_output_snoop(group, key);
            }
        }
        Vec::new()
    }

    pub fn snapshot_all_output_snoops(&self) -> Vec<(u8, u8, Vec<f32>)> {
        #[cfg(feature = "cpal_audio")]
        {
            if let Some(ctrl) = self.stream_controller.read().as_ref() {
                return ctrl.snapshot_all_output_snoops();
            }
        }
        Vec::new()
    }
}
