use std::collections::BTreeMap;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use common::{
    device::DeviceData,
    egui_helpers::{
        enum_combo, find_selected_device, selected_device_label as helper_selected_device_label,
        show_action_error_messages, show_device_combo, show_frequency_spectrum_chart, StatusTone,
    },
    error::{AppError, AudioAnalysisError},
    playback_quality::PlaybackQuality,
};
use eframe::egui::{self};
use fundsp::prelude::*;
use parking_lot::Mutex;

use crate::{
    create_telemetry_channel,
    rt::{
        engine::Engine,
        telemetry::{Message as TelemetryMessage, TelemetryReceiver},
        ExcitementSource,
    },
    PlaybackQualityGate,
};

const DEFAULT_EXPERIMENT_FREQ_HZ: f32 = 440.0;
const DIAGNOSTIC_WARMUP_MS: u64 = 180;
const TELEMETRY_LOG_INTERVAL_MS: u64 = 1000;
const SPECTRUM_WORKER_POLL_INTERVAL_MS: u64 = 16;
const SPECTRUM_WORKER_LOG_INTERVAL_MS: u64 = 1000;
const RAW_ENERGY_LIVE_THRESHOLD: f32 = 5.0e-5;
const RAW_ENERGY_LIVE_HOLD_MS: u64 = 220;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RuntimeTransportState {
    #[default]
    Stopped,
    Running,
    Paused,
}

impl RuntimeTransportState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Stopped => "Stopped",
            Self::Running => "Running",
            Self::Paused => "Paused",
        }
    }
}

#[derive(Clone, Debug, Default)]
struct DiagnosticSnapshot {
    input_len: usize,
    left_spectrum: BTreeMap<u32, f32>,
    right_spectrum: BTreeMap<u32, f32>,
    left_max: f32,
    right_max: f32,
    raw_output_peak: f32,
    raw_energy_live: bool,
    fft_peak_live: bool,
}

pub struct RuntimeTestbed {
    engine: Arc<Engine>,
    telemetry_rx: TelemetryReceiver,
    latest_telemetry: Option<TelemetryMessage>,
    latest_telemetry_received_at: Option<Instant>,
    latest_telemetry_drain_count: usize,
    last_telemetry_log_at: Option<Instant>,
    devices: Vec<DeviceData>,
    output_choice: Option<String>,
    input_choice: Option<String>,
    quality: PlaybackQuality,
    source: ExcitementSource,
    transport: RuntimeTransportState,
    diagnostics: Option<DiagnosticSnapshot>,
    spectrum_shared: Arc<Mutex<SpectrumSharedState>>,
    spectrum_worker: Option<SpectrumWorker>,
    last_spectrum_worker_log_at: Option<Instant>,
    last_network_loaded_at: Option<Instant>,
    diagnostics_status: String,
    startup_network_builder: Option<Box<dyn Fn(f64) -> Net + Send + Sync>>,
    last_action: Option<String>,
    last_error: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct SpectrumSharedState {
    diagnostics: Option<DiagnosticSnapshot>,
    status: String,
    updated_at: Option<Instant>,
    active_trigger_id: u64,
    active_trigger_at: Option<Instant>,
    first_raw_live_trigger_id: u64,
    first_raw_live_at: Option<Instant>,
    first_raw_live_render_trigger_id: u64,
    first_raw_live_render_at: Option<Instant>,
    last_raw_output_peak: f32,
    last_raw_energy_live: bool,
    last_fft_peak_live: bool,
}

struct SpectrumWorker {
    should_stop: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<()>>,
}

impl SpectrumWorker {
    fn stop(&mut self) {
        self.should_stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Default for RuntimeTestbed {
    fn default() -> Self {
        let (engine, telemetry_rx) = build_engine(None, None);
        let mut app = Self {
            engine,
            telemetry_rx,
            latest_telemetry: None,
            latest_telemetry_received_at: None,
            latest_telemetry_drain_count: 0,
            last_telemetry_log_at: None,
            devices: Vec::new(),
            output_choice: None,
            input_choice: None,
            quality: PlaybackQuality::Medium,
            source: ExcitementSource::Entropy,
            transport: RuntimeTransportState::Stopped,
            diagnostics: None,
            spectrum_shared: Arc::new(Mutex::new(SpectrumSharedState {
                diagnostics: None,
                status: "idle".to_string(),
                updated_at: None,
                ..Default::default()
            })),
            spectrum_worker: None,
            last_spectrum_worker_log_at: None,
            last_network_loaded_at: None,
            diagnostics_status: "idle".to_string(),
            startup_network_builder: None,
            last_action: None,
            last_error: None,
        };
        app.refresh_devices();
        app.apply_quality_if_possible();
        app
    }
}

impl RuntimeTestbed {
    pub fn transport_state(&self) -> RuntimeTransportState {
        self.transport
    }

    pub fn start_transport(&mut self) {
        self.handle_start();
    }

    pub fn pause_transport(&mut self) {
        self.handle_pause();
    }

    pub fn resume_transport(&mut self) {
        self.handle_resume();
    }

    pub fn stop_transport(&mut self) {
        self.handle_stop();
    }

    pub fn render_runtime_messages(&self, ui: &mut egui::Ui) {
        show_action_error_messages(ui, &self.last_action, &self.last_error);
    }

    pub fn set_startup_network_builder<F>(&mut self, builder: F)
    where
        F: Fn(f64) -> Net + Send + Sync + 'static,
    {
        self.startup_network_builder = Some(Box::new(builder));
    }

    pub fn register_visual_trigger(&mut self, trigger_id: u64, triggered_at: Instant) {
        let mut shared = self.spectrum_shared.lock();
        shared.active_trigger_id = trigger_id;
        shared.active_trigger_at = Some(triggered_at);
        shared.first_raw_live_trigger_id = 0;
        shared.first_raw_live_at = None;
        shared.first_raw_live_render_trigger_id = 0;
        shared.first_raw_live_render_at = None;
        log::debug!(
            "spectrum_trigger_diag trigger_id={} trigger_age_ms={:.1}",
            trigger_id,
            triggered_at.elapsed().as_secs_f64() * 1000.0,
        );
    }

    pub fn apply_custom_network<F>(&mut self, action_label: impl Into<String>, factory: F)
    where
        F: FnOnce(f64) -> Result<Net, String>,
    {
        let sample_rate = self.current_sample_rate();
        let net = match factory(sample_rate) {
            Ok(net) => net,
            Err(err) => {
                self.set_error(err);
                return;
            }
        };

        self.apply_network(net, action_label.into());
    }

    pub fn render_controls(&mut self, ui: &mut egui::Ui) {
        let previous_source = self.source;

        ui.heading("Runtime controls");
        ui.label("Minimal control surface for Engine + RuntimeSubsystem demos.");

        egui::CollapsingHeader::new("Status")
            .id_salt("runtime_status_section")
            .default_open(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Transport");
                    let tone = match self.transport {
                        RuntimeTransportState::Stopped => StatusTone::Idle,
                        RuntimeTransportState::Running => StatusTone::Success,
                        RuntimeTransportState::Paused => StatusTone::Warning,
                    };
                    ui.colored_label(tone.color(), self.transport.label());
                });
                ui.small(format!(
                    "Requested sample rate for diagnostics: {:.0} Hz",
                    self.current_sample_rate()
                ));
            });

        egui::CollapsingHeader::new("Transport")
            .id_salt("runtime_transport_section")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            self.transport == RuntimeTransportState::Stopped,
                            egui::Button::new("Start"),
                        )
                        .clicked()
                    {
                        self.handle_start();
                    }

                    if ui
                        .add_enabled(
                            self.transport == RuntimeTransportState::Running,
                            egui::Button::new("Pause"),
                        )
                        .clicked()
                    {
                        self.handle_pause();
                    }

                    if ui
                        .add_enabled(
                            self.transport == RuntimeTransportState::Paused,
                            egui::Button::new("Resume"),
                        )
                        .clicked()
                    {
                        self.handle_resume();
                    }

                    if ui
                        .add_enabled(
                            self.transport != RuntimeTransportState::Stopped,
                            egui::Button::new("Stop"),
                        )
                        .clicked()
                    {
                        self.handle_stop();
                    }
                });
            });

        egui::CollapsingHeader::new("Source and quality")
            .id_salt("runtime_source_quality_section")
            .default_open(false)
            .show(ui, |ui| {
                enum_combo(
                    ui,
                    "Excitement source",
                    "runtime_source_combo",
                    &mut self.source,
                    &[
                        (
                            ExcitementSource::Entropy,
                            source_label(ExcitementSource::Entropy),
                        ),
                        (ExcitementSource::Mic, source_label(ExcitementSource::Mic)),
                    ],
                );

                enum_combo(
                    ui,
                    "Playback quality",
                    "runtime_quality_combo",
                    &mut self.quality,
                    &quality_options().map(|quality| (quality, quality_label(quality))),
                );

                if ui.button("Apply quality").clicked() {
                    self.apply_quality_if_possible();
                }
            });

        egui::CollapsingHeader::new("Experiment graphs")
            .id_salt("runtime_graphs_section")
            .default_open(false)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Load experiment graph").clicked() {
                        self.apply_startup_network();
                    }
                    if ui.button("Load silence graph").clicked() {
                        self.apply_silence_network();
                    }
                });
                ui.small("Use these to switch the RuntimeSubsystem inner network for the demo.");
            });

        egui::CollapsingHeader::new("Devices")
            .id_salt("runtime_devices_section")
            .default_open(false)
            .show(ui, |ui| {
                if ui.button("Refresh device list").clicked() {
                    self.refresh_devices();
                    self.set_action("Refreshed CPAL device list");
                }

                show_device_combo(
                    ui,
                    "Output device",
                    "runtime_output_device",
                    &self.devices,
                    &mut self.output_choice,
                    |device| device.supports_output,
                );
                if ui.button("Apply output device").clicked() {
                    self.apply_output_selection();
                }

                show_device_combo(
                    ui,
                    "Input device",
                    "runtime_input_device",
                    &self.devices,
                    &mut self.input_choice,
                    |device| device.supports_input,
                );
                if ui.button("Apply input device").clicked() {
                    self.apply_input_selection();
                }
            });

        egui::CollapsingHeader::new("Messages")
            .id_salt("runtime_messages_section")
            .default_open(self.last_error.is_some())
            .show(ui, |ui| {
                show_action_error_messages(ui, &self.last_action, &self.last_error);
                if self.last_action.is_none() && self.last_error.is_none() {
                    ui.small("No messages yet.");
                }
            });

        self.handle_source_change(previous_source);
    }

    pub fn render_diagnostics(&mut self, ui: &mut egui::Ui) {
        self.refresh_telemetry();
        self.sync_spectrum_worker();

        if self.transport == RuntimeTransportState::Running {
            self.refresh_diagnostics_from_worker();
            ui.ctx().request_repaint();
        }

        ui.heading("Runtime diagnostics");
        ui.label("Live snapshot of RuntimeSubsystem input and output spectrum.");

        egui::CollapsingHeader::new("Status")
            .id_salt("runtime_diagnostics_status_section")
            .default_open(false)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("Selected quality: {}", self.quality_label()));
                    ui.label(format!("Source: {}", source_label(self.source)));
                    ui.label(format!("Diagnostics: {}", self.diagnostics_status));
                });

                if let Some(telemetry) = self.latest_telemetry {
                    let sample_rate = self.current_sample_rate();
                    let queue_before_ms = frames_to_ms(
                        telemetry.queue_depth_frames_before_fill,
                        sample_rate,
                    );
                    let queue_after_ms =
                        frames_to_ms(telemetry.queue_depth_frames_after_fill, sample_rate);
                    let target_ms = frames_to_ms(telemetry.target_buffer_frames, sample_rate);
                    let callback_age_ms = telemetry.callback_started_at.elapsed().as_secs_f64() * 1000.0;
                    let latest_rx_age_ms = self
                        .latest_telemetry_received_at
                        .map(|at| at.elapsed().as_secs_f64() * 1000.0)
                        .unwrap_or_default();

                    ui.small(format!(
                        "Callback queue before/after fill: {:.1} / {:.1} ms (target {:.1} ms)",
                        queue_before_ms, queue_after_ms, target_ms
                    ));
                    ui.small(format!(
                        "Produced {} frames, callback age {:.1} ms, telemetry receive age {:.1} ms (last drain {})",
                        telemetry.produced_frames,
                        callback_age_ms,
                        latest_rx_age_ms,
                        self.latest_telemetry_drain_count
                    ));
                    if let Some(snapshot) = &self.diagnostics {
                        ui.small(format!(
                            "Raw peak {:.3e}, raw_live={}, fft_live={}",
                            snapshot.raw_output_peak,
                            snapshot.raw_energy_live,
                            snapshot.fft_peak_live,
                        ));
                    }
                } else {
                    ui.small("Telemetry: waiting for callback messages");
                }
            });

        egui::CollapsingHeader::new("Configuration")
            .id_salt("runtime_diagnostics_config_section")
            .default_open(false)
            .show(ui, |ui| {
                ui.label(format!(
                    "Output device: {}",
                    selected_device_label(self.selected_output())
                ));
                ui.label(format!(
                    "Input device: {}",
                    selected_device_label(self.selected_input())
                ));
                ui.label(format!("Transport: {}", self.transport.label()));
                ui.label(format!("Known devices: {}", self.devices.len()));
            });

        egui::CollapsingHeader::new("Snapshot")
            .id_salt("runtime_diagnostics_snapshot_section")
            .default_open(true)
            .show(ui, |ui| {
                if let Some(snapshot) = &self.diagnostics {
                    ui.label(format!(
                        "Max L/R: {:.3e} / {:.3e}",
                        snapshot.left_max, snapshot.right_max
                    ));

                    egui::CollapsingHeader::new("Left spectrum")
                        .id_salt("runtime_left_spectrum_section")
                        .default_open(true)
                        .show(ui, |ui| {
                            show_frequency_spectrum_chart(
                                ui,
                                "runtime_left_spectrum_plot",
                                &snapshot.left_spectrum,
                                "No left-channel spectrum yet",
                                egui::Color32::from_rgb(120, 195, 255),
                            );
                        });

                    egui::CollapsingHeader::new("Right spectrum")
                        .id_salt("runtime_right_spectrum_section")
                        .default_open(true)
                        .show(ui, |ui| {
                            show_frequency_spectrum_chart(
                                ui,
                                "runtime_right_spectrum_plot",
                                &snapshot.right_spectrum,
                                "No right-channel spectrum yet",
                                egui::Color32::from_rgb(102, 204, 122),
                            );
                        });
                } else {
                    ui.label("No diagnostics captured yet.");
                }
            });
    }

    fn refresh_devices(&mut self) {
        self.devices = self.engine.list_devices();
    }

    fn current_sample_rate(&self) -> f64 {
        PlaybackQualityGate::from(self.quality).sample_rate(None) as f64
    }

    fn selected_output(&self) -> Option<DeviceData> {
        find_selected_device(&self.devices, self.output_choice.as_deref()).cloned()
    }

    fn selected_input(&self) -> Option<DeviceData> {
        find_selected_device(&self.devices, self.input_choice.as_deref()).cloned()
    }

    fn rebuild_engine(&mut self) {
        self.stop_spectrum_worker();
        let (engine, telemetry_rx) = build_engine(self.selected_output(), self.selected_input());
        self.engine = engine;
        self.telemetry_rx = telemetry_rx;
        self.latest_telemetry = None;
        self.latest_telemetry_received_at = None;
        self.latest_telemetry_drain_count = 0;
        self.last_telemetry_log_at = None;
        self.last_spectrum_worker_log_at = None;
        *self.spectrum_shared.lock() = SpectrumSharedState {
            diagnostics: None,
            status: "idle".to_string(),
            updated_at: None,
            ..Default::default()
        };
        self.apply_quality_if_possible();
    }

    fn refresh_telemetry(&mut self) {
        let mut drained = 0;
        while let Ok(message) = self.telemetry_rx.try_recv() {
            self.latest_telemetry = Some(message);
            self.latest_telemetry_received_at = Some(Instant::now());
            drained += 1;
        }

        if drained > 0 {
            self.latest_telemetry_drain_count = drained;
        }

        if self.transport == RuntimeTransportState::Running {
            if let Some(telemetry) = self.latest_telemetry {
                let now = Instant::now();
                let should_log = self.last_telemetry_log_at.is_none_or(|last| {
                    now.duration_since(last) >= Duration::from_millis(TELEMETRY_LOG_INTERVAL_MS)
                });

                if should_log {
                    self.last_telemetry_log_at = Some(now);
                    let sample_rate = self.current_sample_rate();
                    let queue_before_ms =
                        frames_to_ms(telemetry.queue_depth_frames_before_fill, sample_rate);
                    let queue_after_ms =
                        frames_to_ms(telemetry.queue_depth_frames_after_fill, sample_rate);
                    let target_ms = frames_to_ms(telemetry.target_buffer_frames, sample_rate);
                    let callback_age_ms =
                        telemetry.callback_started_at.elapsed().as_secs_f64() * 1000.0;
                    let telemetry_rx_age_ms = self
                        .latest_telemetry_received_at
                        .map(|at| at.elapsed().as_secs_f64() * 1000.0)
                        .unwrap_or_default();

                    log::debug!(
                        "latency_diag queue_before_ms={:.1} queue_after_ms={:.1} target_ms={:.1} produced_frames={} callback_age_ms={:.1} rx_age_ms={:.1} drain_count={} mode={:?}",
                        queue_before_ms,
                        queue_after_ms,
                        target_ms,
                        telemetry.produced_frames,
                        callback_age_ms,
                        telemetry_rx_age_ms,
                        self.latest_telemetry_drain_count,
                        telemetry.mode,
                    );
                }
            }
        }
    }

    fn set_error(&mut self, error: impl ToString) {
        self.last_error = Some(error.to_string());
    }

    fn clear_error(&mut self) {
        self.last_error = None;
    }

    fn set_action(&mut self, action: impl ToString) {
        self.last_action = Some(action.to_string());
        self.clear_error();
    }

    fn apply_quality_if_possible(&mut self) {
        match self.engine.on_quality_change(self.quality) {
            Ok(()) => {
                self.set_action(format!("Applied {} quality", self.quality_label()));
                if self.transport == RuntimeTransportState::Running {
                    self.stop_spectrum_worker();
                    self.start_spectrum_worker();
                }
            }
            Err(err) => self.set_error(err),
        }
    }

    fn handle_start(&mut self) {
        self.rebuild_engine();

        match self.engine.start(self.source) {
            Ok(()) => {
                self.transport = RuntimeTransportState::Running;
                self.start_spectrum_worker();
                self.set_action(format!(
                    "Started runtime with {} source",
                    source_label(self.source)
                ));
            }
            Err(err) => self.set_error(err),
        }
    }

    fn handle_pause(&mut self) {
        match self.engine.pause() {
            Ok(()) => {
                self.transport = RuntimeTransportState::Paused;
                self.stop_spectrum_worker();
                self.set_action("Paused output stream");
            }
            Err(err) => self.set_error(err),
        }
    }

    fn handle_resume(&mut self) {
        match self.engine.resume() {
            Ok(()) => {
                self.transport = RuntimeTransportState::Running;
                self.start_spectrum_worker();
                self.set_action("Resumed output stream");
            }
            Err(err) => self.set_error(err),
        }
    }

    fn handle_stop(&mut self) {
        match self.engine.stop() {
            Ok(()) => {
                self.transport = RuntimeTransportState::Stopped;
                self.stop_spectrum_worker();
                self.diagnostics = None;
                self.set_action("Stopped runtime");
            }
            Err(err) => self.set_error(err),
        }
    }

    fn apply_startup_network(&mut self) {
        let sample_rate = self.current_sample_rate();
        let net = if let Some(builder) = &self.startup_network_builder {
            builder(sample_rate)
        } else {
            default_experiment_net(sample_rate)
        };

        self.apply_network(net, format!("Loaded experiment graph",));
    }

    fn apply_silence_network(&mut self) {
        let sample_rate = self.current_sample_rate();
        self.apply_network(
            silent_stereo_net(sample_rate),
            "Loaded silent passthrough graph".to_string(),
        );
    }

    fn apply_network(&mut self, net: Net, success_message: String) {
        let mut replaced = false;
        let result = self.engine.with_runtime(|rt| {
            rt.set_inner_network(net);
            replaced = true;
            Ok(())
        });

        match result {
            Ok(()) if replaced => {
                self.stop_spectrum_worker();
                self.last_network_loaded_at = Some(Instant::now());
                self.diagnostics = None;
                self.diagnostics_status = "warming up".to_string();
                *self.spectrum_shared.lock() = SpectrumSharedState {
                    diagnostics: None,
                    status: "warming up".to_string(),
                    updated_at: None,
                    ..Default::default()
                };
                if self.transport == RuntimeTransportState::Running {
                    self.start_spectrum_worker();
                }
                self.set_action(success_message);
            }
            Ok(()) => self.set_error("Runtime is not active yet"),
            Err(err) => self.set_error(err),
        }
    }

    fn refresh_diagnostics_from_worker(&mut self) {
        if self.transport != RuntimeTransportState::Running {
            self.diagnostics_status = "stopped".to_string();
            return;
        }

        if let Some(loaded_at) = self.last_network_loaded_at {
            let elapsed = loaded_at.elapsed();
            if elapsed < Duration::from_millis(DIAGNOSTIC_WARMUP_MS) {
                self.diagnostics = None;
                self.diagnostics_status = format!(
                    "warming up ({:03} ms)",
                    (Duration::from_millis(DIAGNOSTIC_WARMUP_MS) - elapsed).as_millis()
                );
                return;
            }
        }

        let now = Instant::now();
        let mut shared = self.spectrum_shared.lock();
        if shared.first_raw_live_trigger_id == shared.active_trigger_id
            && shared.first_raw_live_trigger_id != 0
            && shared.first_raw_live_render_trigger_id != shared.active_trigger_id
        {
            shared.first_raw_live_render_trigger_id = shared.active_trigger_id;
            shared.first_raw_live_render_at = Some(now);
        }

        self.diagnostics = shared.diagnostics.clone();
        self.diagnostics_status = if shared.status.is_empty() {
            "waiting for worker".to_string()
        } else {
            shared.status.clone()
        };

        let should_log = self.last_spectrum_worker_log_at.is_none_or(|last| {
            now.duration_since(last) >= Duration::from_millis(SPECTRUM_WORKER_LOG_INTERVAL_MS)
        });
        if should_log {
            self.last_spectrum_worker_log_at = Some(now);
            let age_ms = shared
                .updated_at
                .map(|at| at.elapsed().as_secs_f64() * 1000.0)
                .unwrap_or(-1.0);
            let marker_ms = match (shared.active_trigger_at, shared.first_raw_live_at) {
                (Some(triggered_at), Some(first_live_at)) => {
                    first_live_at.duration_since(triggered_at).as_secs_f64() * 1000.0
                }
                _ => -1.0,
            };
            log::debug!(
                "spectrum_ui_diag status={} has_snapshot={} snapshot_age_ms={:.1} raw_peak={:.3e} raw_live={} fft_live={} marker_ms={:.1} worker_running={}",
                self.diagnostics_status,
                self.diagnostics.is_some(),
                age_ms,
                shared.last_raw_output_peak,
                shared.last_raw_energy_live,
                shared.last_fft_peak_live,
                marker_ms,
                self.spectrum_worker.is_some(),
            );
        }
    }

    fn sync_spectrum_worker(&mut self) {
        match (
            self.transport == RuntimeTransportState::Running,
            self.spectrum_worker.is_some(),
        ) {
            (true, false) => self.start_spectrum_worker(),
            (false, true) => self.stop_spectrum_worker(),
            _ => {}
        }
    }

    fn start_spectrum_worker(&mut self) {
        if self.spectrum_worker.is_some() {
            return;
        }

        *self.spectrum_shared.lock() = SpectrumSharedState {
            diagnostics: None,
            status: "waiting for audio".to_string(),
            updated_at: None,
            ..Default::default()
        };

        let engine = self.engine.clone();
        let shared = self.spectrum_shared.clone();
        let sample_rate = self.current_sample_rate();
        let should_stop = Arc::new(AtomicBool::new(false));
        let should_stop_for_thread = should_stop.clone();

        let spawn_result = thread::Builder::new()
            .name("runtime_spectrum_worker".to_string())
            .spawn(move || {
                let poll_interval = Duration::from_millis(SPECTRUM_WORKER_POLL_INTERVAL_MS);
                let mut last_log_at: Option<Instant> = None;
                let mut last_raw_live_at: Option<Instant> = None;

                while !should_stop_for_thread.load(Ordering::Relaxed) {
                    let mut worker_snapshot: Option<DiagnosticSnapshot> = None;
                    let mut worker_status = "runtime unavailable".to_string();
                    let mut worker_raw_peak = 0.0_f32;
                    let mut worker_raw_live = false;
                    let mut worker_fft_live = false;

                    let result = engine.with_runtime(|rt| {
                        let input_len = rt.input_snapshot().len();
                        let output = rt.output_diagnostics(sample_rate, 20.0, 20_000.0)?;
                        worker_raw_peak = output.raw_output_peak;
                        worker_fft_live = output.left_fft_peak > f32::EPSILON
                            || output.right_fft_peak > f32::EPSILON;

                        let compute_now = Instant::now();
                        if worker_raw_peak > RAW_ENERGY_LIVE_THRESHOLD {
                            last_raw_live_at = Some(compute_now);
                        }
                        worker_raw_live = last_raw_live_at.is_some_and(|last| {
                            compute_now.duration_since(last)
                                <= Duration::from_millis(RAW_ENERGY_LIVE_HOLD_MS)
                        });

                        let snapshot = DiagnosticSnapshot {
                            input_len,
                            left_spectrum: output.left_spectrum.clone(),
                            right_spectrum: output.right_spectrum.clone(),
                            left_max: max_peak(&output.left_spectrum),
                            right_max: max_peak(&output.right_spectrum),
                            raw_output_peak: worker_raw_peak,
                            raw_energy_live: worker_raw_live,
                            fft_peak_live: worker_fft_live,
                        };

                        worker_status = if worker_raw_live {
                            "live".to_string()
                        } else if worker_fft_live {
                            "fft-active, raw-low".to_string()
                        } else {
                            "stale or silent".to_string()
                        };
                        worker_snapshot = Some(snapshot);
                        Ok(())
                    });

                    match result {
                        Ok(()) => {
                            if worker_snapshot.is_none() {
                                worker_status = "runtime unavailable".to_string();
                            }
                        }
                        Err(err) => {
                            if matches!(
                                err,
                                AppError::AudioAnalysis(AudioAnalysisError::EmptyOutputBuffer)
                            ) {
                                worker_status = "waiting for audio".to_string();
                            } else {
                                worker_status = "analysis error".to_string();
                                log::debug!("spectrum_worker_diag analysis_error={}", err);
                            }
                        }
                    }

                    let now = Instant::now();
                    {
                        let mut shared = shared.lock();
                        if worker_raw_live
                            && shared.active_trigger_id != 0
                            && shared.first_raw_live_trigger_id != shared.active_trigger_id
                        {
                            shared.first_raw_live_trigger_id = shared.active_trigger_id;
                            shared.first_raw_live_at = Some(now);
                        }
                        shared.diagnostics = worker_snapshot;
                        shared.status = worker_status.clone();
                        shared.updated_at = Some(now);
                        shared.last_raw_output_peak = worker_raw_peak;
                        shared.last_raw_energy_live = worker_raw_live;
                        shared.last_fft_peak_live = worker_fft_live;
                    }

                    let should_log = last_log_at.is_none_or(|last| {
                        now.duration_since(last)
                            >= Duration::from_millis(SPECTRUM_WORKER_LOG_INTERVAL_MS)
                    });
                    if should_log {
                        last_log_at = Some(now);
                        let shared = shared.lock();
                        let bins = shared
                            .diagnostics
                            .as_ref()
                            .map(|d| d.left_spectrum.len())
                            .unwrap_or_default();
                        let marker_ms = match (shared.active_trigger_at, shared.first_raw_live_at) {
                            (Some(triggered_at), Some(first_live_at)) => {
                                first_live_at.duration_since(triggered_at).as_secs_f64() * 1000.0
                            }
                            _ => -1.0,
                        };
                        log::debug!(
                            "spectrum_worker_diag poll_interval_ms={} status={} has_snapshot={} left_bins={} raw_peak={:.3e} raw_live={} fft_live={} marker_ms={:.1}",
                            SPECTRUM_WORKER_POLL_INTERVAL_MS,
                            shared.status,
                            shared.diagnostics.is_some(),
                            bins,
                            shared.last_raw_output_peak,
                            shared.last_raw_energy_live,
                            shared.last_fft_peak_live,
                            marker_ms,
                        );
                    }

                    thread::sleep(poll_interval);
                }

                log::debug!("spectrum_worker_diag stopped=true");
            });

        match spawn_result {
            Ok(handle) => {
                self.spectrum_worker = Some(SpectrumWorker {
                    should_stop,
                    join_handle: Some(handle),
                });
                log::debug!(
                    "spectrum_worker_diag started=true poll_interval_ms={}",
                    SPECTRUM_WORKER_POLL_INTERVAL_MS
                );
            }
            Err(err) => {
                log::error!("spectrum_worker_diag started=false error={}", err);
            }
        }
    }

    fn stop_spectrum_worker(&mut self) {
        if let Some(mut worker) = self.spectrum_worker.take() {
            worker.stop();
            log::debug!("spectrum_worker_diag stop_requested=true");
        }
    }

    fn apply_output_selection(&mut self) {
        if self.transport == RuntimeTransportState::Stopped {
            self.set_action("Output device selection will apply on next start");
            return;
        }

        let Some(device) = self.selected_output() else {
            self.set_error("Select an output device first");
            return;
        };

        match self
            .engine
            .select_output_device(device.host_id.clone(), device.device_id.clone())
        {
            Ok(()) => {
                self.transport = RuntimeTransportState::Running;
                self.set_action(format!("Applied output device: {device}"));
            }
            Err(err) => self.set_error(err),
        }
    }

    fn apply_input_selection(&mut self) {
        if self.transport != RuntimeTransportState::Stopped && self.source != ExcitementSource::Mic
        {
            self.set_action("Input device selection will apply when starting with Mic source");
            return;
        }

        if self.transport == RuntimeTransportState::Stopped {
            self.set_action("Input device selection will apply on next start");
            return;
        }

        let Some(device) = self.selected_input() else {
            self.set_error("Select an input device first");
            return;
        };

        match self
            .engine
            .select_input_device(device.host_id.clone(), device.device_id.clone())
        {
            Ok(()) => self.set_action(format!("Applied input device: {device}")),
            Err(err) => self.set_error(err),
        }
    }

    fn handle_source_change(&mut self, previous: ExcitementSource) {
        if self.transport == RuntimeTransportState::Stopped || previous == self.source {
            return;
        }

        match self.engine.on_excitement_source_changed(self.source) {
            Ok(()) => self.set_action(format!(
                "Switched excitement source to {}",
                source_label(self.source)
            )),
            Err(err) => self.set_error(err),
        }
    }

    fn quality_label(&self) -> &'static str {
        quality_label(self.quality)
    }
}

fn build_engine(
    output_device: Option<DeviceData>,
    input_device: Option<DeviceData>,
) -> (Arc<Engine>, TelemetryReceiver) {
    let (telemetry, telemetry_rx) = create_telemetry_channel();
    (
        Arc::new(Engine::new(telemetry, output_device, input_device)),
        telemetry_rx,
    )
}

impl Drop for RuntimeTestbed {
    fn drop(&mut self) {
        self.stop_spectrum_worker();
    }
}

fn default_experiment_net(sample_rate: f64) -> Net {
    let mut net = Net::new(1, 2);
    let input_id = net.push(Box::new(sink()));
    let tone_id = net.push(Box::new(
        sine_hz::<f32>(DEFAULT_EXPERIMENT_FREQ_HZ) >> split::<U2>(),
    ));

    net.connect_input(0, input_id, 0);
    net.connect_output(tone_id, 0, 0);
    net.connect_output(tone_id, 1, 1);
    net.set_sample_rate(sample_rate);
    net.check();
    net
}

fn silent_stereo_net(sample_rate: f64) -> Net {
    let mut net = Net::new(1, 2);
    let input_id = net.push(Box::new(sink()));
    let silence_id = net.push(Box::new(constant::<Frame<f32, U2>>(Frame::default())));

    net.connect_input(0, input_id, 0);
    net.connect_output(silence_id, 0, 0);
    net.connect_output(silence_id, 1, 1);
    net.set_sample_rate(sample_rate);
    net.check();
    net
}

fn max_peak(spectrum: &BTreeMap<u32, f32>) -> f32 {
    spectrum
        .values()
        .copied()
        .fold(0.0_f32, |acc, value| acc.max(value))
}

fn selected_device_label(device: Option<DeviceData>) -> String {
    helper_selected_device_label(device.as_ref())
}

fn source_label(source: ExcitementSource) -> &'static str {
    match source {
        ExcitementSource::Entropy => "Entropy",
        ExcitementSource::Mic => "Mic",
    }
}

fn quality_options() -> [PlaybackQuality; 5] {
    [
        PlaybackQuality::Auto(0),
        PlaybackQuality::LoFi,
        PlaybackQuality::Medium,
        PlaybackQuality::HiFi,
        PlaybackQuality::Ultra,
    ]
}

fn quality_label(quality: PlaybackQuality) -> &'static str {
    match quality {
        PlaybackQuality::Auto(_) => "Auto",
        PlaybackQuality::LoFi => "LoFi",
        PlaybackQuality::Medium => "Medium",
        PlaybackQuality::HiFi => "HiFi",
        PlaybackQuality::Ultra => "Ultra",
    }
}

fn frames_to_ms(frames: usize, sample_rate: f64) -> f64 {
    if sample_rate <= 0.0 {
        0.0
    } else {
        (frames as f64 / sample_rate) * 1000.0
    }
}
