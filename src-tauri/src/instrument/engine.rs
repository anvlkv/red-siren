use parking_lot::RwLock;

use audio_system::rt::{make_stream_controller, ActivationSource, StreamController};

use common::instrument::{Config as InstrumentConfig, Layout as InstrumentLayout};
use common::tuner::Config as TunerConfig;
use mint::Vector2;
use tauri::{AppHandle, Manager};

/// Public wrapper so higher layers (commands) only hold one handle.
pub struct InstrumentEngine {
    app: AppHandle,
    pub(super) inner: Inner,
}

impl InstrumentEngine {
    pub fn new(app: &AppHandle) -> Self {
        InstrumentEngine {
            app: app.clone(),
            inner: Inner {
                playing: RwLock::new(false),
                activation_source: RwLock::new(ActivationSource::default()),
                layout: RwLock::new(InstrumentLayout::default()),
                config: RwLock::new(InstrumentConfig::default()),
                stream_controller: RwLock::new(None),
            },
        }
    }

    pub fn layout(&self) -> InstrumentLayout {
        self.inner.layout()
    }

    pub fn playing(&self) -> bool {
        self.inner.playing()
    }

    pub fn activation_source(&self) -> ActivationSource {
        self.inner.activation_source()
    }

    pub fn start_playback(&self) -> common::error::Result<bool> {
        let tuner_state = self.app.state::<crate::tuner::TunerState>();
        let tuner_config = tuner_state.tuner_config.read();
        {
            log::trace!("InstrumentEngine.start_playback()");
            let res = self.inner.start_playback(&tuner_config);
            if let Ok(changed) = &res {
                log::trace!("InstrumentEngine.start_playback -> {}", changed);
            } else {
                log::error!(
                    "InstrumentEngine.start_playback error: {:?}",
                    res.as_ref().err()
                );
            }
            res
        }
    }

    pub fn stop_playback(&self) -> common::error::Result<bool> {
        {
            log::trace!("InstrumentEngine.stop_playback()");
            let res = self.inner.stop_playback();
            if let Ok(changed) = &res {
                log::trace!("InstrumentEngine.stop_playback -> {}", changed);
            } else {
                log::error!(
                    "InstrumentEngine.stop_playback error: {:?}",
                    res.as_ref().err()
                );
            }
            res
        }
    }

    pub fn pause_playback(&self) -> common::error::Result<bool> {
        {
            log::trace!("InstrumentEngine.pause_playback()");
            let res = self.inner.pause_playback();
            if let Ok(changed) = &res {
                log::trace!("InstrumentEngine.pause_playback -> {}", changed);
            } else {
                log::error!(
                    "InstrumentEngine.pause_playback error: {:?}",
                    res.as_ref().err()
                );
            }
            res
        }
    }

    pub fn resume_playback(&self) -> common::error::Result<bool> {
        {
            log::trace!("InstrumentEngine.resume_playback()");
            let res = self.inner.resume_playback();
            if let Ok(changed) = &res {
                log::trace!("InstrumentEngine.resume_playback -> {}", changed);
            } else {
                log::error!(
                    "InstrumentEngine.resume_playback error: {:?}",
                    res.as_ref().err()
                );
            }
            res
        }
    }

    pub fn set_activation_source(&self, src: ActivationSource) -> common::error::Result<bool> {
        let tuner_state = self.app.state::<crate::tuner::TunerState>();
        let tuner_config = tuner_state.tuner_config.read();
        {
            log::trace!("InstrumentEngine.set_activation_source({:?})", src);
            let res = self.inner.set_activation_source(src, &tuner_config);
            if let Ok(changed) = &res {
                log::trace!(
                    "InstrumentEngine.set_activation_source -> changed={}",
                    changed
                );
            } else {
                log::error!(
                    "InstrumentEngine.set_activation_source error: {:?}",
                    res.as_ref().err()
                );
            }
            res
        }
    }

    pub fn set_is_dark(&self, is_dark: bool) -> common::error::Result<()> {
        let tuner_state = self.app.state::<crate::tuner::TunerState>();
        let tuner_config = tuner_state.tuner_config.read();
        self.inner.set_is_dark(is_dark, &tuner_config)
    }

    pub fn set_size(&self, width: f64, height: f64) -> common::error::Result<()> {
        let tuner_state = self.app.state::<crate::tuner::TunerState>();
        let tuner_config = tuner_state.tuner_config.read();
        self.inner.set_size(width, height, &tuner_config)
    }

    pub fn set_safe_area(
        &self,
        top: f32,
        right: f32,
        bottom: f32,
        left: f32,
    ) -> common::error::Result<()> {
        let tuner_state = self.app.state::<crate::tuner::TunerState>();
        let tuner_config = tuner_state.tuner_config.read();
        self.inner
            .set_safe_area(top, right, bottom, left, &tuner_config)
    }

    pub fn snapshot_output_snoop(&self, group: usize, key: usize) -> Vec<f32> {
        self.inner.snapshot_output_snoop(group, key)
    }

    pub fn snapshot_all_output_snoops(&self) -> Vec<(u8, u8, Vec<f32>)> {
        self.inner.snapshot_all_output_snoops()
    }

    pub fn snapshot_activation_snoop(&self, group: usize, key: usize) -> Vec<f32> {
        self.inner.snapshot_activation_snoop(group, key)
    }

    pub fn snapshot_all_activation_snoops(&self) -> Vec<(u8, u8, Vec<f32>)> {
        self.inner.snapshot_all_activation_snoops()
    }

    pub fn set_band_control(&self, key: common::NodeKey, value: f32) -> common::error::Result<()> {
        self.inner.set_band_control(key, value)
    }

    pub fn get_band_control(&self, key: common::NodeKey) -> common::error::Result<f32> {
        self.inner.get_band_control(key)
    }
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

impl Inner {
    // ---------------------------------------------------------------------
    // Accessors

    fn layout(&self) -> InstrumentLayout {
        *self.layout.read()
    }

    fn playing(&self) -> bool {
        *self.playing.read()
    }

    fn activation_source(&self) -> ActivationSource {
        *self.activation_source.read()
    }

    // ---------------------------------------------------------------------
    // Playback lifecycle
    // ---------------------------------------------------------------------

    fn start_playback(&self, tuner_config: &TunerConfig) -> common::error::Result<bool> {
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
                log::trace!(
                    "Inner.start_playback: starting stream with source={:?}",
                    source
                );
                ctrl.start(&layout, &config, source, tuner_config)?;
                log::trace!("Inner.start_playback: ctrl.start() returned Ok");
            }
        }

        Ok(true)
    }

    fn stop_playback(&self) -> common::error::Result<bool> {
        if !*self.playing.read() {
            return Ok(false);
        }

        if let Some(ctrl) = self.stream_controller.read().as_ref() {
            // Even if stop errors, proceed to mark stopped for consistency
            log::trace!("Inner.stop_playback: calling ctrl.stop()");
            let _ = ctrl.stop();
            log::trace!("Inner.stop_playback: ctrl.stop() returned");
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

    fn pause_playback(&self) -> common::error::Result<bool> {
        if !*self.playing.read() {
            return Ok(false);
        }

        if let Some(ctrl) = self.stream_controller.read().as_ref() {
            log::trace!("Inner.pause_playback: calling ctrl.pause()");
            ctrl.pause()?;
            log::trace!("Inner.pause_playback: ctrl.pause() returned");
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

    fn resume_playback(&self) -> common::error::Result<bool> {
        if *self.playing.read() {
            return Ok(false);
        }

        if let Some(ctrl) = self.stream_controller.read().as_ref() {
            log::trace!("Inner.resume_playback: calling ctrl.resume()");
            ctrl.resume()?;
            log::trace!("Inner.resume_playback: ctrl.resume() returned");
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

    fn set_activation_source(
        &self,
        src: ActivationSource,
        tuner_config: &TunerConfig,
    ) -> common::error::Result<bool> {
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
            log::trace!("Inner.set_activation_source: changed to {:?}", src);
            if let Some(ctrl) = self.stream_controller.read().as_ref() {
                log::trace!("Inner.set_activation_source: notifying stream controller");
                ctrl.on_activation_source_changed(src)?;

                // Also trigger layout change to ensure proper system recreation with new tuner config
                let layout = self.layout.read();
                let config = self.config.read();
                ctrl.on_layout_changed(&layout, &config, tuner_config)?;
                log::info!("Recreated audio systems after activation source change");
            }
        } else {
            log::trace!("Inner.set_activation_source: no-op (already {:?})", src);
        }

        Ok(changed)
    }

    // ---------------------------------------------------------------------
    // Layout / Config maintenance
    // ---------------------------------------------------------------------

    fn set_is_dark(&self, is_dark: bool, tuner_config: &TunerConfig) -> common::error::Result<()> {
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
            ctrl.on_layout_changed(&layout, &config, tuner_config)?;
            log::info!("Recreated audio systems after dark mode change");
        }

        Ok(())
    }

    fn set_size(
        &self,
        width: f64,
        height: f64,
        tuner_config: &TunerConfig,
    ) -> common::error::Result<()> {
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
            ctrl.on_layout_changed(&layout, &config, tuner_config)?;
            log::info!("Recreated audio systems after size change");
        }

        Ok(())
    }

    fn set_safe_area(
        &self,
        top: f32,
        right: f32,
        bottom: f32,
        left: f32,
        tuner_config: &TunerConfig,
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
            ctrl.on_layout_changed(&layout, &config, tuner_config)?;
            log::info!("Recreated audio systems after safe area change");
        }

        Ok(())
    }

    // ---------------------------------------------------------------------
    // Data taps
    // ---------------------------------------------------------------------

    fn snapshot_output_snoop(&self, group: usize, key: usize) -> Vec<f32> {
        if let Some(ctrl) = self.stream_controller.read().as_ref() {
            return ctrl.snapshot_output_snoop(group, key);
        }
        Vec::new()
    }

    fn snapshot_all_output_snoops(&self) -> Vec<(u8, u8, Vec<f32>)> {
        if let Some(ctrl) = self.stream_controller.read().as_ref() {
            return ctrl.snapshot_all_output_snoops();
        }
        Vec::new()
    }

    fn snapshot_activation_snoop(&self, group: usize, key: usize) -> Vec<f32> {
        if let Some(ctrl) = self.stream_controller.read().as_ref() {
            return ctrl.snapshot_activation_snoop(group, key);
        }
        Vec::new()
    }

    fn snapshot_all_activation_snoops(&self) -> Vec<(u8, u8, Vec<f32>)> {
        if let Some(ctrl) = self.stream_controller.read().as_ref() {
            return ctrl.snapshot_all_activation_snoops();
        }
        Vec::new()
    }

    fn set_band_control(&self, key: common::NodeKey, value: f32) -> common::error::Result<()> {
        if let Some(ctrl) = self.stream_controller.read().as_ref() {
            return ctrl.set_band_control(key, value);
        }
        Ok(())
    }

    fn get_band_control(&self, key: common::NodeKey) -> common::error::Result<f32> {
        if let Some(ctrl) = self.stream_controller.read().as_ref() {
            return ctrl.get_band_control(key);
        }
        Ok(0.0)
    }
}
