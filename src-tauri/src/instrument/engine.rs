use common::NodeKey;
use parking_lot::RwLock;

use audio_system::rt::{make_stream_controller, AudioRuntime, ExcitementSource};

use common::error::{AppError, InstrumentError, Result};
use common::instrument::{
    Config as InstrumentConfig, Layout as InstrumentLayout, PlaybackQuality, Preset,
};
use common::tuner::Config as TunerConfig;
use mint::Vector2;
use tauri::{AppHandle, Emitter, Manager};

/// Public wrapper so higher layers (commands) only hold one handle.
pub struct InstrumentEngine {
    app: AppHandle,
    inner: Inner,
}

/// Internal engine state.
pub(super) struct Inner {
    // Playback & instrument state
    playing: RwLock<bool>,
    excitement_source: RwLock<ExcitementSource>,
    layout: RwLock<InstrumentLayout>,
    config: RwLock<InstrumentConfig>,

    // Runtime stream controller (lazy; concrete backend chosen by audio_system::rt)
    stream_controller: RwLock<Box<dyn AudioRuntime + Send + Sync>>,
}

impl InstrumentEngine {
    pub fn new(app: &AppHandle) -> Result<Self> {
        Ok(InstrumentEngine {
            app: app.clone(),
            inner: Inner {
                playing: RwLock::new(false),
                excitement_source: RwLock::new(ExcitementSource::default()),
                layout: RwLock::new(InstrumentLayout::default()),
                config: RwLock::new(InstrumentConfig::default()),
                stream_controller: RwLock::new(make_stream_controller()?),
            },
        })
    }

    pub fn layout(&self) -> InstrumentLayout {
        *self.inner.layout.read()
    }

    pub fn playing(&self) -> bool {
        *self.inner.playing.read()
    }

    pub fn excitement_source(&self) -> ExcitementSource {
        *self.inner.excitement_source.read()
    }

    pub fn start_playback(&self) -> common::error::Result<bool> {
        let tuner_state = self.app.state::<crate::tuner::TunerState>();
        let tuner_config = tuner_state.tuner_config();
        {
            log::trace!("InstrumentEngine.start_playback()");
            if *self.inner.playing.read() {
                return Ok(false);
            }

            {
                let mut p = self.inner.playing.write();
                if *p {
                    return Ok(false);
                }
                *p = true;
            }

            {
                let ctrl = self.inner.stream_controller.read();

                let layout = self.inner.layout.read();
                let config = self.inner.config.read();
                let source = *self.inner.excitement_source.read();
                log::trace!(
                    "Inner.start_playback: starting stream with source={:?}",
                    source
                );
                ctrl.start(&layout, &config, source, &tuner_config)?;
                log::trace!("Inner.start_playback: ctrl.start() returned Ok");
            }

            // After the stream has started, emit reflect values based on the current preset
            {
                // Snapshot layout to avoid holding a read lock during emits
                let layout_snapshot = *self.inner.layout.read();
                self.app
                    .emit(common::instrument::events::LAYOUT, layout_snapshot)
                    .map_err(|e| {
                        AppError::Instrument(InstrumentError::Emit {
                            event: common::instrument::events::LAYOUT.to_string(),
                            message: e.to_string(),
                        })
                    })?;

                let preset = self.get_preset();
                for node_key in layout_snapshot.registry().all_keys() {
                    let band = preset.get_band_value(&node_key).unwrap_or(0.0);
                    let key = preset.get_key_value(&node_key).unwrap_or(0.0);
                    self.app
                        .emit(
                            common::instrument::events::BAND_CONTROL_G_K,
                            common::instrument::commands::ReflectBandControlPayload {
                                group: node_key.group(),
                                key: node_key.key(),
                                value: band,
                            },
                        )
                        .map_err(|e| {
                            AppError::Instrument(InstrumentError::Emit {
                                event: common::instrument::events::BAND_CONTROL_G_K.to_string(),
                                message: e.to_string(),
                            })
                        })?;
                    self.app
                        .emit(
                            common::instrument::events::KEY_CONTROL_G_K,
                            common::instrument::commands::ReflectKeyControlPayload {
                                group: node_key.group(),
                                key: node_key.key(),
                                value: key,
                            },
                        )
                        .map_err(|e| {
                            AppError::Instrument(InstrumentError::Emit {
                                event: common::instrument::events::KEY_CONTROL_G_K.to_string(),
                                message: e.to_string(),
                            })
                        })?;
                }
            }

            Ok(true)
        }
    }

    pub fn stop_playback(&self) -> common::error::Result<bool> {
        log::trace!("InstrumentEngine.stop_playback()");
        if !*self.inner.playing.read() {
            return Ok(false);
        }

        {
            let mut p = self.inner.playing.write();
            if !*p {
                return Ok(false);
            }
            *p = false;
        }

        // Even if stop errors, proceed to mark stopped for consistency
        log::trace!("Inner.stop_playback: calling ctrl.stop()");
        let _ = self.inner.stream_controller.read().stop();
        log::trace!("Inner.stop_playback: ctrl.stop() returned");

        Ok(true)
    }

    pub fn pause_playback(&self) -> common::error::Result<bool> {
        log::trace!("InstrumentEngine.pause_playback()");
        if !*self.inner.playing.read() {
            return Ok(false);
        }

        {
            let mut p = self.inner.playing.write();
            if !*p {
                return Ok(false);
            }
            *p = false;
        }

        log::trace!("Inner.pause_playback: calling ctrl.pause()");
        self.inner.stream_controller.read().pause()?;
        log::trace!("Inner.pause_playback: ctrl.pause() returned");

        Ok(true)
    }

    pub fn resume_playback(&self) -> common::error::Result<bool> {
        log::trace!("InstrumentEngine.resume_playback()");
        if *self.inner.playing.read() {
            return Ok(false);
        }

        {
            let mut p = self.inner.playing.write();
            if *p {
                return Ok(false);
            }
            *p = true;
        }

        log::trace!("Inner.resume_playback: calling ctrl.resume()");
        self.inner.stream_controller.read().resume()?;
        log::trace!("Inner.resume_playback: ctrl.resume() returned");

        Ok(true)
    }

    pub fn set_excitement_source(&self, src: ExcitementSource) -> common::error::Result<bool> {
        let tuner_state = self.app.state::<crate::tuner::TunerState>();
        let tuner_config = tuner_state.tuner_config();
        log::trace!("InstrumentEngine.set_excitement_source({:?})", src);
        let changed = {
            let mut current = self.inner.excitement_source.write();
            if *current != src {
                *current = src;
                true
            } else {
                false
            }
        };

        if changed {
            log::trace!("Inner.set_excitement_source: changed to {:?}", src);
            let ctrl = self.inner.stream_controller.read();
            log::trace!("Inner.set_excitement_source: notifying stream controller");
            ctrl.on_excitement_source_changed(src)?;

            // Also trigger layout change to ensure proper system recreation with new tuner config
            // Snapshot state to avoid holding read locks during controller calls
            let layout_snapshot = *self.inner.layout.read();
            let config_snapshot = InstrumentConfig::try_from(layout_snapshot)?;
            ctrl.on_layout_changed(&layout_snapshot, &config_snapshot, &tuner_config)?;
            log::info!("Recreated audio systems after excitement source change");
        } else {
            log::trace!("Inner.set_excitement_source: no-op (already {:?})", src);
        }

        self.app
            .emit(
                common::instrument::events::EXCITEMENT_SRC,
                common::instrument::events::ExcitementSourcePayload { source: src.into() },
            )
            .map_err(|e| common::error::InstrumentError::Emit {
                event: common::instrument::events::EXCITEMENT_SRC.to_string(),
                message: e.to_string(),
            })?;

        Ok(changed)
    }

    pub fn set_is_dark(&self, is_dark: bool) -> common::error::Result<()> {
        let tuner_state = self.app.state::<crate::tuner::TunerState>();
        let tuner_config = tuner_state.tuner_config();
        // Derive new config with minimal lock hold time
        let new_cfg = {
            let mut layout = self.inner.layout.write();
            layout.scale = if is_dark {
                common::instrument::Scale::In
            } else {
                common::instrument::Scale::Yo
            };
            InstrumentConfig::try_from(*layout)?
        };
        {
            let mut cfg = self.inner.config.write();
            *cfg = new_cfg;
        }

        // Snapshot state to avoid holding read locks during controller calls
        let layout_snapshot = *self.inner.layout.read();
        let config_snapshot = InstrumentConfig::try_from(layout_snapshot)?;
        self.inner.stream_controller.read().on_layout_changed(
            &layout_snapshot,
            &config_snapshot,
            &tuner_config,
        )?;
        log::info!("Recreated audio systems after dark mode change");

        Ok(())
    }

    pub fn set_size(&self, width: f64, height: f64) -> common::error::Result<()> {
        let tuner_state = self.app.state::<crate::tuner::TunerState>();
        let tuner_config = tuner_state.tuner_config();
        let new_cfg = {
            let mut layout = self.inner.layout.write();
            let scale = layout.scale;
            *layout = InstrumentLayout {
                scale,
                ..InstrumentLayout::from_screen_estate(Vector2 {
                    x: width,
                    y: height,
                })
            };
            InstrumentConfig::try_from(*layout)?
        };
        {
            let mut cfg = self.inner.config.write();
            *cfg = new_cfg;
        }

        // Snapshot state to avoid holding read locks during controller calls
        let layout_snapshot = *self.inner.layout.read();
        let config_snapshot = InstrumentConfig::try_from(layout_snapshot)?;
        self.inner.stream_controller.read().on_layout_changed(
            &layout_snapshot,
            &config_snapshot,
            &tuner_config,
        )?;
        log::info!("Recreated audio systems after size change");

        Ok(())
    }

    pub fn set_preset(&self, preset: Preset) -> common::error::Result<()> {
        self.inner.stream_controller.read().set_preset(preset)
    }

    pub fn set_safe_area(
        &self,
        top: f64,
        right: f64,
        bottom: f64,
        left: f64,
    ) -> common::error::Result<()> {
        let tuner_state = self.app.state::<crate::tuner::TunerState>();
        let tuner_config = tuner_state.tuner_config();

        let new_cfg = {
            let mut layout = self.inner.layout.write();
            let space = layout.space;
            let scale = layout.scale;
            *layout = InstrumentLayout::from_screen_estate_with_safe_area(
                space, top, right, bottom, left,
            );
            layout.scale = scale;
            InstrumentConfig::try_from(*layout)?
        };
        {
            let mut cfg = self.inner.config.write();
            *cfg = new_cfg;
        }

        // Snapshot state to avoid holding read locks during controller calls
        let layout_snapshot = *self.inner.layout.read();
        let config_snapshot = InstrumentConfig::try_from(layout_snapshot)?;
        self.inner.stream_controller.read().on_layout_changed(
            &layout_snapshot,
            &config_snapshot,
            &tuner_config,
        )?;
        log::info!("Recreated audio systems after safe area change");

        Ok(())
    }

    pub fn set_band_control(&self, key: common::NodeKey, value: f32) -> common::error::Result<()> {
        self.inner
            .stream_controller
            .read()
            .set_band_control(key, value)
    }

    pub fn get_band_control(&self, key: common::NodeKey) -> common::error::Result<f32> {
        self.inner.stream_controller.read().get_band_control(key)
    }

    pub fn set_key_control(&self, key: common::NodeKey, value: f32) -> common::error::Result<()> {
        self.inner
            .stream_controller
            .read()
            .set_key_control(key, value)
    }

    pub fn get_key_control(&self, key: common::NodeKey) -> common::error::Result<f32> {
        self.inner.stream_controller.read().get_key_control(key)
    }

    pub fn snapshot_input_snoop(&self) -> Vec<f32> {
        self.inner.stream_controller.read().snapshot_input_snoop()
    }

    pub fn snapshot_output_snoop(&self, key: NodeKey) -> Vec<f32> {
        self.inner
            .stream_controller
            .read()
            .snapshot_output_snoop(key)
    }

    pub fn snapshot_all_output_snoops(&self) -> Vec<(NodeKey, Vec<f32>)> {
        self.inner
            .stream_controller
            .read()
            .snapshot_all_output_snoops()
    }

    pub fn snapshot_excitement_snoop(&self, key: NodeKey) -> Vec<(f32, f32)> {
        self.inner
            .stream_controller
            .read()
            .snapshot_excitement_snoop(key)
    }

    pub fn snapshot_all_excitement_snoops(&self) -> Vec<(NodeKey, Vec<(f32, f32)>)> {
        self.inner
            .stream_controller
            .read()
            .snapshot_all_excitement_snoops()
    }

    pub fn snapshot_processed_output_spectrum(&self) -> Option<Vec<(f32, f32)>> {
        match self
            .inner
            .stream_controller
            .read()
            .snapshot_processed_output_spectrum()
        {
            Ok(data) => data.map(|(left, right)| {
                if left.len() != right.len() {
                    log::warn!(
                        "Mismatched left-to-right [{}]/[{}] spectrum lengths",
                        left.len(),
                        right.len()
                    );
                }
                left.into_values()
                    .zip(right.into_values().rev())
                    .rev()
                    .collect::<Vec<(f32, f32)>>()
            }),
            Err(e) => {
                log::error!("Error snapshotting processed output spectrum: {}", e);
                None
            }
        }
    }

    pub fn sample_rate(&self) -> f64 {
        self.inner.stream_controller.read().get_sample_rate()
    }

    pub fn poll_spectrum(&self) -> Option<common::tuner::SpectrumSnapshot> {
        self.inner.stream_controller.read().poll_tuner_spectrum()
    }

    pub fn poll_excitements(&self) -> Vec<(NodeKey, f32)> {
        self.inner.stream_controller.read().poll_tuner_excitements()
    }

    pub fn start_tuner_only_stream(&self, tuner_config: &TunerConfig) -> Result<()> {
        self.inner
            .stream_controller
            .read()
            .start_tuner_only(tuner_config)
    }

    pub fn stop_tuner_only_stream(&self) -> Result<()> {
        self.inner.stream_controller.read().stop()
    }

    pub fn update_tuner_config(&self, tuner_config: &TunerConfig) -> Result<()> {
        self.inner
            .stream_controller
            .read()
            .update_tuner_config(tuner_config)
    }

    pub fn start_tap_tuner_audio(&self) -> Result<()> {
        self.inner.stream_controller.read().start_tap_tuner_audio()
    }

    pub fn stop_tap_tuner_audio(&self) -> Result<()> {
        self.inner.stream_controller.read().stop_tap_tuner_audio()
    }

    pub fn quality_indicator(&self) -> PlaybackQuality {
        self.inner.stream_controller.read().quality_indicator()
    }

    pub fn get_preset(&self) -> Preset {
        self.inner.stream_controller.read().get_preset()
    }

    #[cfg(feature = "devtools")]
    pub fn get_finetuned_values(
        &self,
    ) -> common::error::Result<common::commands::edit::FineTunedValuesPayload> {
        self.inner.stream_controller.read().get_finetuned_values()
    }

    #[cfg(feature = "devtools")]
    pub fn set_finetuned_values(
        &self,
        payload: common::commands::edit::FineTunedValuesPayload,
    ) -> common::error::Result<()> {
        self.inner
            .stream_controller
            .read()
            .set_finetuned_values(payload)
    }
}
