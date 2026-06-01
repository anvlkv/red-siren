use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use common::{
    device::DeviceData,
    egui_helpers::{
        enum_combo, find_selected_device, selected_device_label as helper_selected_device_label,
        show_action_error_messages, show_device_combo, show_frequency_spectrum_chart, StatusTone,
    },
    playback_quality::PlaybackQuality,
};
use eframe::egui::{self};
use fundsp::prelude::*;

use crate::{
    create_telemetry_channel,
    rt::{engine::Engine, telemetry::TelemetryReceiver, ExcitementSource},
    PlaybackQualityGate,
};

const DEFAULT_EXPERIMENT_FREQ_HZ: f32 = 440.0;
const DIAGNOSTIC_WARMUP_MS: u64 = 180;

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
}

pub struct RuntimeTestbed {
    engine: Engine,
    _telemetry_rx: TelemetryReceiver,
    devices: Vec<DeviceData>,
    output_choice: Option<String>,
    input_choice: Option<String>,
    quality: PlaybackQuality,
    source: ExcitementSource,
    transport: RuntimeTransportState,
    diagnostics: Option<DiagnosticSnapshot>,
    last_network_loaded_at: Option<Instant>,
    diagnostics_status: String,
    startup_network_builder: Option<Box<dyn Fn(f64) -> Net + Send + Sync>>,
    last_action: Option<String>,
    last_error: Option<String>,
}

impl Default for RuntimeTestbed {
    fn default() -> Self {
        let (engine, telemetry_rx) = build_engine(None, None);
        let mut app = Self {
            engine,
            _telemetry_rx: telemetry_rx,
            devices: Vec::new(),
            output_choice: None,
            input_choice: None,
            quality: PlaybackQuality::Medium,
            source: ExcitementSource::Entropy,
            transport: RuntimeTransportState::Stopped,
            diagnostics: None,
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
        if self.transport == RuntimeTransportState::Running {
            self.refresh_diagnostics();
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
                    ui.label(format!("Input samples captured: {}", snapshot.input_len));
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
        let (engine, telemetry_rx) = build_engine(self.selected_output(), self.selected_input());
        self.engine = engine;
        self._telemetry_rx = telemetry_rx;
        self.apply_quality_if_possible();
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
            Ok(()) => self.set_action(format!("Applied {} quality", self.quality_label())),
            Err(err) => self.set_error(err),
        }
    }

    fn handle_start(&mut self) {
        self.rebuild_engine();

        match self.engine.start(self.source) {
            Ok(()) => {
                self.transport = RuntimeTransportState::Running;
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
                self.set_action("Paused output stream");
            }
            Err(err) => self.set_error(err),
        }
    }

    fn handle_resume(&mut self) {
        match self.engine.resume() {
            Ok(()) => {
                self.transport = RuntimeTransportState::Running;
                self.set_action("Resumed output stream");
            }
            Err(err) => self.set_error(err),
        }
    }

    fn handle_stop(&mut self) {
        match self.engine.stop() {
            Ok(()) => {
                self.transport = RuntimeTransportState::Stopped;
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
                self.last_network_loaded_at = Some(Instant::now());
                self.diagnostics = None;
                self.diagnostics_status = "warming up".to_string();
                self.set_action(success_message);
            }
            Ok(()) => self.set_error("Runtime is not active yet"),
            Err(err) => self.set_error(err),
        }
    }

    fn refresh_diagnostics(&mut self) {
        if self.transport != RuntimeTransportState::Running {
            self.diagnostics_status = "stopped".to_string();
            return;
        }

        if let Some(loaded_at) = self.last_network_loaded_at {
            let elapsed = loaded_at.elapsed();
            if elapsed < Duration::from_millis(DIAGNOSTIC_WARMUP_MS) {
                self.diagnostics_status = format!(
                    "warming up ({:03} ms)",
                    (Duration::from_millis(DIAGNOSTIC_WARMUP_MS) - elapsed).as_millis()
                );
                return;
            }
        }

        let sample_rate = self.current_sample_rate();
        let mut snapshot = None;

        let result = self.engine.with_runtime(|rt| {
            let input_len = rt.input_snapshot().len();
            let (left, right) = rt.output_spectrum(sample_rate, 20.0, 20_000.0)?;
            snapshot = Some(DiagnosticSnapshot {
                input_len,
                left_spectrum: left.clone(),
                right_spectrum: right.clone(),
                left_max: max_peak(&left),
                right_max: max_peak(&right),
            });
            Ok(())
        });

        match result {
            Ok(()) => {
                if let Some(snapshot) = snapshot {
                    if snapshot.left_max <= f32::EPSILON && snapshot.right_max <= f32::EPSILON {
                        self.diagnostics_status = "stale or silent".to_string();
                        self.diagnostics = Some(snapshot);
                    } else {
                        self.diagnostics = Some(snapshot);
                        self.diagnostics_status = "live".to_string();
                    }
                } else {
                    self.diagnostics_status = "runtime unavailable".to_string();
                    self.set_error("Runtime is not active yet");
                }
            }
            Err(err) => {
                self.diagnostics_status = "analysis error".to_string();
                self.set_error(err)
            }
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
) -> (Engine, TelemetryReceiver) {
    let (telemetry, telemetry_rx) = create_telemetry_channel();
    (
        Engine::new(telemetry, output_device, input_device),
        telemetry_rx,
    )
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
