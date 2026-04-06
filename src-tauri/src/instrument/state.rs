use std::sync::Arc;

use audio_system::create_telemetry_channel;
use audio_system::rt::gate_manager::QualityGateManager;
use audio_system::PlaybackQualityGate;
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

use crate::instrument::{load_input_device, load_output_device, load_preset};

/// Public wrapper so higher layers (commands) only hold one handle.
pub struct InstrumentState {
    app: AppHandle,
    inner: Inner,
    /// Manages quality gate telemetry and user quality preference.
    gate_manager: Arc<QualityGateManager>,
}

/// Internal engine state.
pub(super) struct Inner {
    // Playback & instrument state
    playing: RwLock<bool>,
    excitement_source: RwLock<ExcitementSource>,
    layout: RwLock<InstrumentLayout>,
    config: RwLock<InstrumentConfig>,
    resize_locked: RwLock<bool>,
    // Runtime stream controller (lazy; concrete backend chosen by audio_system::rt)
    stream_controller: Arc<RwLock<Box<dyn AudioRuntime + Send + Sync>>>,
}

impl InstrumentState {
    pub fn new(app: &AppHandle) -> Result<Self> {
        let input_device = load_input_device(app)?;
        let output_device = load_output_device(app)?;
        let preset = load_preset(app).ok();
        let initial_layout = InstrumentLayout::from_screen_estate(Vector2 {
            x: 1280.0,
            y: 720.0,
        });
        let initial_config = InstrumentConfig::try_from(initial_layout).unwrap_or_else(|err| {
            log::error!(
                "Failed to derive instrument config from default layout during InstrumentState init: {err}"
            );
            InstrumentConfig::default()
        });
        let (telemetry_sx, telemetry_rx) = create_telemetry_channel();
        let stream_controller = Arc::new(RwLock::new(make_stream_controller(
            telemetry_sx,
            preset,
            output_device,
            input_device,
        )?));
        let gate_manager = QualityGateManager::new(
            telemetry_rx,
            Box::new({
                let stream_controller = stream_controller.clone();
                move |gate| {
                    // Check whether the sample type is changing before we commit the update.
                    let current_type = stream_controller.read().current_sample_type();
                    let new_type = gate.sample_type();

                    log::debug!(
                        "managed_change: applying auto quality gate {:?} (sample_type: {:?} → {:?})",
                        gate,
                        current_type,
                        new_type
                    );

                    // Always update the gate immediately (affects buffer-size adaptation
                    // in the hot path with zero latency).
                    stream_controller.read().update_quality_setting(gate);

                    // A sample-type change (F32 ↔ F64) requires a full DSP network rebuild.
                    // Spawn a blocking thread so we don't stall the GateWorker async loop.
                    if new_type != current_type && stream_controller.read().is_running() {
                        log::info!(
                            "managed_change: sample type changed {:?} → {:?}; scheduling stream restart",
                            current_type,
                            new_type
                        );
                        let sc = stream_controller.clone();
                        log::info!("managed_change: restarting stream for quality-driven sample-type change");
                        std::thread::spawn(move || {
                            if let Err(e) = sc.read().restart_for_quality() {
                                log::error!("Quality-driven sample-type restart failed: {e}");
                            } else {
                                log::info!("managed_change: stream restart completed successfully");
                            }
                        });
                    }
                }
            }),
        );
        Ok(InstrumentState {
            app: app.clone(),
            gate_manager: Arc::new(gate_manager),
            inner: Inner {
                playing: RwLock::new(false),
                excitement_source: RwLock::new(ExcitementSource::default()),
                layout: RwLock::new(initial_layout),
                config: RwLock::new(initial_config),
                resize_locked: RwLock::new(false),
                stream_controller,
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

    pub fn is_resize_locked(&self) -> bool {
        *self.inner.resize_locked.read()
    }

    pub fn set_resize_locked(&self, locked: bool) {
        *self.inner.resize_locked.write() = locked;
    }

    pub fn start_playback(&self) -> common::error::Result<bool> {
        let tuner_state = self.app.state::<crate::tuner::TunerState>();
        let tuner_config = tuner_state.tuner_config();
        {
            log::trace!("InstrumentState.start_playback()");
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
                let preset = load_preset(&self.app)?;
                log::trace!(
                    "Inner.start_playback: starting stream with source={:?}",
                    source
                );
                ctrl.start(&layout, &config, source, &tuner_config, preset)?;
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
        log::trace!("InstrumentState.stop_playback()");
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
        log::trace!("InstrumentState.pause_playback()");
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
        log::trace!("InstrumentState.resume_playback()");
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
        log::trace!("InstrumentState.set_excitement_source({:?})", src);
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
        if self.is_resize_locked() {
            log::debug!(
                "Skipping layout size update because resize lock is enabled ({}x{})",
                width,
                height
            );
            return Ok(());
        }

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

    pub fn set_layout(&self, layout: InstrumentLayout) -> common::error::Result<()> {
        let tuner_state = self.app.state::<crate::tuner::TunerState>();
        let tuner_config = tuner_state.tuner_config();

        let new_cfg = InstrumentConfig::try_from(layout)?;
        {
            let mut current_layout = self.inner.layout.write();
            *current_layout = layout;
        }
        {
            let mut cfg = self.inner.config.write();
            *cfg = new_cfg;
        }

        let layout_snapshot = *self.inner.layout.read();
        let config_snapshot = InstrumentConfig::try_from(layout_snapshot)?;
        self.inner.stream_controller.read().on_layout_changed(
            &layout_snapshot,
            &config_snapshot,
            &tuner_config,
        )?;
        log::info!("Recreated audio systems after manual layout update");

        Ok(())
    }

    pub fn set_safe_area(
        &self,
        top: f64,
        right: f64,
        bottom: f64,
        left: f64,
    ) -> common::error::Result<()> {
        if self.is_resize_locked() {
            log::debug!(
                "Skipping safe area update because resize lock is enabled (top={}, right={}, bottom={}, left={})",
                top, right, bottom, left
            );
            return Ok(());
        }

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
        self.gate_manager.current_quality()
    }

    pub fn set_quality(&self, quality: PlaybackQuality) {
        log::info!("InstrumentState::set_quality: requested {:?}", quality);
        self.gate_manager.set_quality(quality);
        // For manual (non-Auto) selections, apply the gate to the stream immediately.
        if !matches!(quality, PlaybackQuality::Auto(_)) {
            let gate = PlaybackQualityGate::from(quality);
            let sc = &self.inner.stream_controller;
            let current_type = sc.read().current_sample_type();
            let new_type = gate.sample_type();
            log::debug!(
                "set_quality: applying manual gate {:?} (sample_type: {:?} → {:?})",
                gate,
                current_type,
                new_type
            );
            sc.read().update_quality_setting(gate);
            if new_type != current_type && sc.read().is_running() {
                log::info!(
                    "set_quality: sample type changed {:?} → {:?}; scheduling stream restart",
                    current_type,
                    new_type
                );
                let sc_clone = sc.clone();
                log::info!("set_quality: restarting stream for manual quality change");
                std::thread::spawn(move || {
                    if let Err(e) = sc_clone.read().restart_for_quality() {
                        log::error!("Manual quality restart failed: {e}");
                    } else {
                        log::info!("set_quality: stream restart completed successfully");
                    }
                });
            } else {
                log::debug!(
                    "set_quality: gate applied; no restart needed (running={}, type_changed={})",
                    sc.read().is_running(),
                    new_type != current_type
                );
            }
        } else {
            // On re-entry to Auto mode, immediately apply the gate manager's current
            // proposal so the stream doesn't lag until the next telemetry tick.
            let proposed = PlaybackQualityGate::from(self.gate_manager.proposed_quality());
            let sc = &self.inner.stream_controller;
            let current_type = sc.read().current_sample_type();
            let new_type = proposed.sample_type();
            log::info!(
                "set_quality: switched to Auto mode; applying current proposal {:?} immediately (sample_type: {:?} → {:?})",
                proposed,
                current_type,
                new_type
            );
            sc.read().update_quality_setting(proposed);
            if new_type != current_type && sc.read().is_running() {
                log::info!(
                    "set_quality: Auto re-entry requires sample-type change {:?} → {:?}; scheduling stream restart",
                    current_type,
                    new_type
                );
                let sc_clone = sc.clone();
                log::info!("set_quality: restarting stream for Auto re-entry sample-type change");
                std::thread::spawn(move || {
                    if let Err(e) = sc_clone.read().restart_for_quality() {
                        log::error!("Auto re-entry stream restart failed: {e}");
                    } else {
                        log::info!(
                            "set_quality: Auto re-entry stream restart completed successfully"
                        );
                    }
                });
            }
        }
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
