use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::Instant,
};

use audio_system::{
    egui_testbed::{RuntimeTestbed, RuntimeTransportState},
    system::input_analyzer::InputAnalyzer,
};
use common::egui_helpers::{
    run_native_app, show_frequency_spectrum_chart, show_scrolled_left_panel_inside,
    VisualizationThrottle,
};
use eframe::egui;
use fundsp::prelude::*;

const WINDOW_TITLE: &str = "Input analyzer demo";
const WINDOW_SIZE: [f32; 2] = [1180.0, 760.0];
const ANALYZER_WINDOW_SIZE: usize = 2048;
const ANALYZER_OVERLAP: f64 = 0.75;

fn main() -> eframe::Result<()> {
    run_native_app(
        WINDOW_TITLE,
        WINDOW_SIZE,
        Some("audio_system=debug,input_analyzer_egui=debug"),
        |_cc| {
            let mut app = RuntimeDemoApp {
                testbed: RuntimeTestbed::default(),
                analyzer: InputAnalyzer::new(ANALYZER_WINDOW_SIZE, ANALYZER_OVERLAP),
                sample_rate_hz: Arc::new(AtomicU32::new(48_000)),
                analyzer_spectrum: BTreeMap::new(),
                analyzer_last_snapshot_at: None,
                analyzer_empty_polls: 0,
                spectrum_throttle: VisualizationThrottle::spectrum_default_tagged(
                    "input_analyzer_spectrum",
                ),
            };

            let analyzer = app.analyzer.clone();
            let sample_rate_hz = app.sample_rate_hz.clone();
            app.testbed.set_startup_network_builder(move |sample_rate| {
                sample_rate_hz.store(sample_rate.round() as u32, Ordering::Relaxed);

                let mut net = Net::new(1, 2);
                let analyzer_id = net.push(Box::new(An(analyzer.clone())));

                net.connect_input(0, analyzer_id, 0);
                net.set_sample_rate(sample_rate);
                net.check();
                net
            });
            app
        },
    )
}

struct RuntimeDemoApp {
    testbed: RuntimeTestbed,
    analyzer: InputAnalyzer,
    sample_rate_hz: Arc<AtomicU32>,
    analyzer_spectrum: BTreeMap<u32, f32>,
    analyzer_last_snapshot_at: Option<Instant>,
    analyzer_empty_polls: u64,
    spectrum_throttle: VisualizationThrottle,
}

impl RuntimeDemoApp {
    fn update_input_analyzer_snapshot(&mut self) {
        let is_running = self.testbed.transport_state() == RuntimeTransportState::Running;
        if !is_running {
            self.analyzer_spectrum.clear();
            self.analyzer_last_snapshot_at = None;
            self.analyzer_empty_polls = 0;
            self.spectrum_throttle.reset();
            return;
        }

        if !self.spectrum_throttle.should_refresh() {
            return;
        }

        let mut latest = None;
        while let Some(spectrum) = self.analyzer.get_spectrum() {
            latest = Some(spectrum);
        }

        if let Some(spectrum) = latest {
            self.analyzer_spectrum = map_spectrum_to_hz(
                &spectrum,
                std::cmp::max(self.sample_rate_hz.load(Ordering::Relaxed), 1),
                self.analyzer.window_size(),
            );
            self.analyzer_last_snapshot_at = Some(Instant::now());
            self.analyzer_empty_polls = 0;
        } else {
            self.analyzer_empty_polls = self.analyzer_empty_polls.saturating_add(1);
        }
    }

    fn render_input_analyzer_diagnostics(&self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Input analyzer")
            .id_salt("input_analyzer_status")
            .default_open(true)
            .show(ui, |ui| {
                ui.small(format!(
                    "window: {} | overlap: {:.2} | hop: {}",
                    self.analyzer.window_size(),
                    self.analyzer.overlap(),
                    self.analyzer.hop_size(),
                ));
                ui.small(format!(
                    "sample rate: {} Hz | empty polls: {}",
                    self.sample_rate_hz.load(Ordering::Relaxed),
                    self.analyzer_empty_polls,
                ));
                if let Some(snapshot_at) = self.analyzer_last_snapshot_at {
                    ui.small(format!(
                        "snapshot age: {:.1} ms",
                        snapshot_at.elapsed().as_secs_f64() * 1000.0,
                    ));
                } else {
                    ui.small("snapshot age: n/a");
                }
            });
    }
}

impl eframe::App for RuntimeDemoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.update_input_analyzer_snapshot();

        show_scrolled_left_panel_inside(ui, "runtime_controls", 360.0, None, |ui| {
            self.testbed.render_controls(ui);
            ui.separator();
            self.render_input_analyzer_diagnostics(ui);
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            show_frequency_spectrum_chart(
                ui,
                "input_analyzer_spectrum_chart",
                &self.analyzer_spectrum,
                "No input analyzer spectrum yet",
                egui::Color32::from_rgb(120, 195, 255),
            );

            ui.separator();
            self.testbed.render_diagnostics(ui);
        });

        if self.testbed.transport_state() == RuntimeTransportState::Running {
            ui.ctx().request_repaint();
        }
    }
}

fn map_spectrum_to_hz(
    spectrum: &[Complex32],
    sample_rate_hz: u32,
    window_size: usize,
) -> BTreeMap<u32, f32> {
    let mut mapped = BTreeMap::new();
    if window_size == 0 {
        return mapped;
    }

    let sample_rate = sample_rate_hz as f32;
    let window = window_size as f32;
    for (index, value) in spectrum.iter().enumerate() {
        let frequency_hz = (index as f32 * sample_rate / window).round() as u32;
        mapped.insert(frequency_hz, value.norm());
    }

    mapped
}
