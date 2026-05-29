use std::collections::BTreeMap;

use common::{device::DeviceData, playback_quality::PlaybackQuality};
use eframe::egui::{self, Color32, RichText};
use fundsp::prelude::*;

use crate::{
    create_telemetry_channel,
    rt::{engine::Engine, telemetry::TelemetryReceiver, ExcitementSource},
    PlaybackQualityGate,
};

const STARTER_FREQ_HZ: f32 = 440.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum TransportState {
    #[default]
    Stopped,
    Running,
    Paused,
}

impl TransportState {
    fn label(self) -> &'static str {
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
    left_peaks: Vec<(u32, f32)>,
    right_peaks: Vec<(u32, f32)>,
}

pub struct RuntimeTestbed {
    engine: Engine,
    _telemetry_rx: TelemetryReceiver,
    devices: Vec<DeviceData>,
    output_choice: Option<String>,
    input_choice: Option<String>,
    quality: PlaybackQuality,
    source: ExcitementSource,
    transport: TransportState,
    diagnostics: Option<DiagnosticSnapshot>,
    auto_load_sine_on_start: bool,
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
            transport: TransportState::Stopped,
            diagnostics: None,
            auto_load_sine_on_start: true,
            last_action: None,
            last_error: None,
        };
        app.refresh_devices();
        app.apply_quality_if_possible();
        app
    }
}

impl RuntimeTestbed {
    pub fn render_controls(&mut self, ui: &mut egui::Ui) {
        let previous_source = self.source;

        ui.heading("Runtime gate");
        ui.label("Reusable egui control surface for Engine + RuntimeSubsystem.");
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Status");
            let color = match self.transport {
                TransportState::Stopped => Color32::from_rgb(160, 160, 160),
                TransportState::Running => Color32::from_rgb(102, 204, 122),
                TransportState::Paused => Color32::from_rgb(255, 196, 92),
            };
            ui.colored_label(color, self.transport.label());
        });

        ui.checkbox(
            &mut self.auto_load_sine_on_start,
            "Auto-load starter sine graph",
        );

        ui.separator();
        ui.label(RichText::new("Transport").strong());
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    self.transport == TransportState::Stopped,
                    egui::Button::new("Start"),
                )
                .clicked()
            {
                self.handle_start();
            }

            if ui
                .add_enabled(
                    self.transport == TransportState::Running,
                    egui::Button::new("Pause"),
                )
                .clicked()
            {
                self.handle_pause();
            }

            if ui
                .add_enabled(
                    self.transport == TransportState::Paused,
                    egui::Button::new("Resume"),
                )
                .clicked()
            {
                self.handle_resume();
            }

            if ui
                .add_enabled(
                    self.transport != TransportState::Stopped,
                    egui::Button::new("Stop"),
                )
                .clicked()
            {
                self.handle_stop();
            }
        });

        ui.separator();
        ui.label(RichText::new("Source and quality").strong());

        egui::ComboBox::from_label("Excitement source")
            .selected_text(source_label(self.source))
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.source,
                    ExcitementSource::Entropy,
                    source_label(ExcitementSource::Entropy),
                );
                ui.selectable_value(
                    &mut self.source,
                    ExcitementSource::Mic,
                    source_label(ExcitementSource::Mic),
                );
            });

        egui::ComboBox::from_label("Playback quality")
            .selected_text(self.quality_label())
            .show_ui(ui, |ui| {
                for quality in quality_options() {
                    ui.selectable_value(&mut self.quality, quality, quality_label(quality));
                }
            });

        if ui.button("Apply quality").clicked() {
            self.apply_quality_if_possible();
        }

        ui.small(format!(
            "Requested sample rate for diagnostics: {:.0} Hz",
            self.current_sample_rate()
        ));

        ui.separator();
        ui.label(RichText::new("Starter graph").strong());
        ui.horizontal_wrapped(|ui| {
            if ui.button("Load sine graph").clicked() {
                self.apply_starter_network();
            }
            if ui.button("Load silence graph").clicked() {
                self.apply_silence_network();
            }
        });
        ui.small("The starter graph sinks the runtime input and emits a stereo sine beep.");

        ui.separator();
        ui.label(RichText::new("Devices").strong());
        if ui.button("Refresh device list").clicked() {
            self.refresh_devices();
            self.set_action("Refreshed CPAL device list");
        }

        egui::ComboBox::from_label("Output device")
            .selected_text(selected_device_label(self.selected_output()))
            .show_ui(ui, |ui| {
                for device in self.devices.iter().filter(|device| device.supports_output) {
                    ui.selectable_value(
                        &mut self.output_choice,
                        Some(device_key(device)),
                        device.to_string(),
                    );
                }
            });
        if ui.button("Apply output device").clicked() {
            self.apply_output_selection();
        }

        egui::ComboBox::from_label("Input device")
            .selected_text(selected_device_label(self.selected_input()))
            .show_ui(ui, |ui| {
                for device in self.devices.iter().filter(|device| device.supports_input) {
                    ui.selectable_value(
                        &mut self.input_choice,
                        Some(device_key(device)),
                        device.to_string(),
                    );
                }
            });
        if ui.button("Apply input device").clicked() {
            self.apply_input_selection();
        }

        ui.separator();
        ui.label(RichText::new("Messages").strong());
        if let Some(action) = &self.last_action {
            ui.colored_label(Color32::from_rgb(120, 195, 255), action);
        }
        if let Some(error) = &self.last_error {
            ui.colored_label(Color32::from_rgb(255, 120, 120), error);
        }

        self.handle_source_change(previous_source);
    }

    pub fn render_diagnostics(&mut self, ui: &mut egui::Ui) {
        ui.heading("Runtime diagnostics");
        ui.label("This demo tracks requested quality and selected devices locally because the example does not reach into Engine internals.");
        ui.separator();

        ui.horizontal_wrapped(|ui| {
            if ui.button("Refresh diagnostics").clicked() {
                self.refresh_diagnostics();
            }

            ui.label(format!("Selected quality: {}", self.quality_label()));
            ui.label(format!("Source: {}", source_label(self.source)));
        });

        ui.separator();
        ui.columns(2, |cols| {
            cols[0].group(|ui| {
                ui.label(RichText::new("Configuration").strong());
                ui.label(format!(
                    "Output device: {}",
                    selected_device_label(self.selected_output())
                ));
                ui.label(format!(
                    "Input device: {}",
                    selected_device_label(self.selected_input())
                ));
                ui.label(format!("Transport: {}", self.transport.label()));
                ui.label(format!(
                    "Auto-load sine on start: {}",
                    yes_no(self.auto_load_sine_on_start)
                ));
                ui.label(format!("Known devices: {}", self.devices.len()));
            });

            cols[1].group(|ui| {
                ui.label(RichText::new("Snapshot").strong());
                if let Some(snapshot) = &self.diagnostics {
                    ui.label(format!("Input samples captured: {}", snapshot.input_len));
                    ui.label("Left peaks");
                    render_peaks(ui, &snapshot.left_peaks);
                    ui.separator();
                    ui.label("Right peaks");
                    render_peaks(ui, &snapshot.right_peaks);
                } else {
                    ui.label("No diagnostics captured yet.");
                }
            });
        });
    }

    fn refresh_devices(&mut self) {
        self.devices = self.engine.list_devices();
    }

    fn current_sample_rate(&self) -> f64 {
        PlaybackQualityGate::from(self.quality).sample_rate(None) as f64
    }

    fn selected_output(&self) -> Option<DeviceData> {
        self.output_choice
            .as_ref()
            .and_then(|key| {
                self.devices
                    .iter()
                    .find(|device| device_key(device) == *key)
            })
            .cloned()
    }

    fn selected_input(&self) -> Option<DeviceData> {
        self.input_choice
            .as_ref()
            .and_then(|key| {
                self.devices
                    .iter()
                    .find(|device| device_key(device) == *key)
            })
            .cloned()
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
            Ok(()) => {
                if self.transport != TransportState::Stopped && self.auto_load_sine_on_start {
                    self.apply_starter_network();
                } else {
                    self.set_action(format!("Applied {} quality", self.quality_label()));
                }
            }
            Err(err) => self.set_error(err),
        }
    }

    fn handle_start(&mut self) {
        self.rebuild_engine();

        match self.engine.start(self.source) {
            Ok(()) => {
                self.transport = TransportState::Running;
                self.set_action(format!(
                    "Started runtime with {} source",
                    source_label(self.source)
                ));
                if self.auto_load_sine_on_start {
                    self.apply_starter_network();
                }
            }
            Err(err) => self.set_error(err),
        }
    }

    fn handle_pause(&mut self) {
        match self.engine.pause() {
            Ok(()) => {
                self.transport = TransportState::Paused;
                self.set_action("Paused output stream");
            }
            Err(err) => self.set_error(err),
        }
    }

    fn handle_resume(&mut self) {
        match self.engine.resume() {
            Ok(()) => {
                self.transport = TransportState::Running;
                self.set_action("Resumed output stream");
            }
            Err(err) => self.set_error(err),
        }
    }

    fn handle_stop(&mut self) {
        match self.engine.stop() {
            Ok(()) => {
                self.transport = TransportState::Stopped;
                self.diagnostics = None;
                self.set_action("Stopped runtime");
            }
            Err(err) => self.set_error(err),
        }
    }

    fn apply_starter_network(&mut self) {
        let sample_rate = self.current_sample_rate();
        let mut replaced = false;

        let result = self.engine.with_runtime(|rt| {
            rt.set_inner_network(starter_sine_net(sample_rate));
            replaced = true;
            Ok(())
        });

        match result {
            Ok(()) if replaced => {
                self.set_action(format!(
                    "Loaded starter sine graph at {:.0} Hz",
                    STARTER_FREQ_HZ
                ));
            }
            Ok(()) => self.set_error("Runtime is not active yet"),
            Err(err) => self.set_error(err),
        }
    }

    fn apply_silence_network(&mut self) {
        let sample_rate = self.current_sample_rate();
        let mut replaced = false;

        let result = self.engine.with_runtime(|rt| {
            rt.set_inner_network(silent_stereo_net(sample_rate));
            replaced = true;
            Ok(())
        });

        match result {
            Ok(()) if replaced => self.set_action("Loaded silent passthrough graph"),
            Ok(()) => self.set_error("Runtime is not active yet"),
            Err(err) => self.set_error(err),
        }
    }

    fn refresh_diagnostics(&mut self) {
        let sample_rate = self.current_sample_rate();
        let mut snapshot = None;

        let result = self.engine.with_runtime(|rt| {
            let input_len = rt.input_snapshot().len();
            let (left, right) = rt.output_spectrum(sample_rate, 20.0, 20_000.0)?;
            snapshot = Some(DiagnosticSnapshot {
                input_len,
                left_peaks: strongest_peaks(&left, 5),
                right_peaks: strongest_peaks(&right, 5),
            });
            Ok(())
        });

        match result {
            Ok(()) => {
                if let Some(snapshot) = snapshot {
                    self.diagnostics = Some(snapshot);
                    self.set_action("Refreshed runtime diagnostics");
                } else {
                    self.set_error("Runtime is not active yet");
                }
            }
            Err(err) => self.set_error(err),
        }
    }

    fn apply_output_selection(&mut self) {
        if self.transport == TransportState::Stopped {
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
                self.transport = TransportState::Running;
                self.set_action(format!("Applied output device: {device}"));
                if self.auto_load_sine_on_start {
                    self.apply_starter_network();
                }
            }
            Err(err) => self.set_error(err),
        }
    }

    fn apply_input_selection(&mut self) {
        if self.transport != TransportState::Stopped && self.source != ExcitementSource::Mic {
            self.set_action("Input device selection will apply when starting with Mic source");
            return;
        }

        if self.transport == TransportState::Stopped {
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
        if self.transport == TransportState::Stopped || previous == self.source {
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

fn starter_sine_net(sample_rate: f64) -> Net {
    let mut net = Net::new(1, 2);
    let input_id = net.push(Box::new(sink()));
    let tone_id = net.push(Box::new(sine_hz::<f32>(STARTER_FREQ_HZ) >> split::<U2>()));

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

fn strongest_peaks(spectrum: &BTreeMap<u32, f32>, limit: usize) -> Vec<(u32, f32)> {
    let mut peaks = spectrum
        .iter()
        .map(|(frequency, amplitude)| (*frequency, *amplitude))
        .collect::<Vec<_>>();
    peaks.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.cmp(&right.0))
    });
    peaks.truncate(limit);
    peaks
}

fn render_peaks(ui: &mut egui::Ui, peaks: &[(u32, f32)]) {
    if peaks.is_empty() {
        ui.small("No peaks yet");
        return;
    }

    for (frequency, amplitude) in peaks {
        ui.monospace(format!("{frequency:>5} Hz  {amplitude:.4}"));
    }
}

fn device_key(device: &DeviceData) -> String {
    format!("{}::{}", device.host_id, device.device_id)
}

fn selected_device_label(device: Option<DeviceData>) -> String {
    device
        .map(|device| device.to_string())
        .unwrap_or_else(|| "Default device".to_string())
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

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}
