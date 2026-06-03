use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use audio_system::{
    egui_testbed::{RuntimeTestbed, RuntimeTransportState},
    system::{adsr_3d::adsr_3d, node::Node},
};
use common::egui_helpers::{
    run_native_app, show_buffer_line_chart, show_scrolled_left_panel_inside,
};
use eframe::egui;
use enum2egui::GuiInspect;
use fundsp::prelude::*;
use parking_lot::Mutex;

const WINDOW_TITLE: &str = "Node + ADSR demo";
const WINDOW_SIZE: [f32; 2] = [1180.0, 760.0];
const NODE_SNOOP_WINDOW_SIZE: usize = 256;
const SNAPSHOT_LOG_INTERVAL_MS: u64 = 1000;
const NODE_SNOOP_POLL_INTERVAL_MS: u64 = 5;
const NODE_SNOOP_NONZERO_EPSILON: f32 = 1.0e-6;

fn main() -> eframe::Result<()> {
    run_native_app(
        WINDOW_TITLE,
        WINDOW_SIZE,
        Some("audio_system=debug,node_adsr_egui=debug"),
        |_cc| {
            let node_cfg = common::config::Node {
                key: common::config::NodeKey {
                    key: 0,
                    band_key: 0,
                },
                base_frequency_hz: 300.0,
                path_spacing_hz: 3.0,
                num_modes: 8,
                mode_spacing_hz: 10.0,
                mode_decay_s: 0.2,
            };

            let seq = sequencer::Sequencer::new(0, 1, sequencer::ReplayMode::None);

            let mut app = RuntimeDemoApp {
                testbed: RuntimeTestbed::default(),
                node: Arc::new(Mutex::new(node_cfg)),
                node_rt: Arc::new(Mutex::new(Node::new(node_cfg))),
                seq: Arc::new(Mutex::new(seq)),
                trigger_prev_down: false,
                trigger_seq: 0,
                trigger_last_at: None,
                adsr_dur_0: Arc::new(Shared::new(0.10)),
                adsr_weight_0: Arc::new(Shared::new(1.00)),
                adsr_dur_1: Arc::new(Shared::new(0.18)),
                adsr_weight_1: Arc::new(Shared::new(0.35)),
                adsr_dur_2: Arc::new(Shared::new(0.24)),
                adsr_weight_2: Arc::new(Shared::new(0.00)),
                node_snoop_shared: Arc::new(Mutex::new(NodeSnoopShared::default())),
                node_snoop_worker: None,
                node_snoop_samples: Vec::new(),
                node_snoop_last_snapshot_at: None,
                node_snoop_empty_polls: 0,
                node_snoop_last_ui_log_at: None,
                node_snoop_active_trigger_id: 0,
                node_snoop_first_nonzero_at: None,
                node_snoop_first_ui_render_at: None,
                node_snoop_last_append_len: 0,
            };

            let node_rt = app.node_rt.clone();
            let node_cfg = app.node.clone();
            let seq = app.seq.clone();
            let adsr_dur_0 = app.adsr_dur_0.clone();
            let adsr_weight_0 = app.adsr_weight_0.clone();
            let adsr_dur_1 = app.adsr_dur_1.clone();
            let adsr_weight_1 = app.adsr_weight_1.clone();
            let adsr_dur_2 = app.adsr_dur_2.clone();
            let adsr_weight_2 = app.adsr_weight_2.clone();

            app.testbed.set_startup_network_builder(move |sample_rate| {
                *node_rt.lock() = Node::new(*node_cfg.lock());

                let mut sequencer = sequencer::Sequencer::new(0, 1, sequencer::ReplayMode::None);
                sequencer.set_sample_rate(sample_rate);
                let seq_backend = sequencer.backend();
                *seq.lock() = sequencer;

                let mut net = Net::new(1, 2);
                let input_id = net.push(Box::new(sink()));
                let seq_id = net.push(Box::new(seq_backend));
                let adsr_id = net.push(Box::new(
                    (pass()
                        | (var(&adsr_dur_0)
                            | var(&adsr_weight_0)
                            | var(&adsr_dur_1)
                            | var(&adsr_weight_1)
                            | var(&adsr_dur_2)
                            | var(&adsr_weight_2)))
                        >> adsr_3d::<f32, U3>()
                        >> (dcblock_hz::<f32>(50.0)
                            | dcblock_hz::<f32>(50.0)
                            | dcblock_hz::<f32>(50.0)),
                ));
                let node_id = net.push(Box::new(node_rt.lock().backend()));
                let split_id = net.push(Box::new(split::<U2>()));

                net.connect(seq_id, 0, adsr_id, 0);
                net.pipe_all(adsr_id, node_id);
                net.pipe_all(node_id, split_id);

                net.connect_input(0, input_id, 0);
                net.connect_output(split_id, 0, 0);
                net.connect_output(split_id, 1, 1);
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
    node: Arc<Mutex<common::config::Node>>,
    node_rt: Arc<Mutex<Node<f32>>>,
    seq: Arc<Mutex<sequencer::Sequencer>>,
    trigger_prev_down: bool,
    trigger_seq: u64,
    trigger_last_at: Option<Instant>,
    adsr_dur_0: Arc<Shared>,
    adsr_weight_0: Arc<Shared>,
    adsr_dur_1: Arc<Shared>,
    adsr_weight_1: Arc<Shared>,
    adsr_dur_2: Arc<Shared>,
    adsr_weight_2: Arc<Shared>,
    node_snoop_shared: Arc<Mutex<NodeSnoopShared>>,
    node_snoop_worker: Option<NodeSnoopWorker>,
    node_snoop_samples: Vec<f32>,
    node_snoop_last_snapshot_at: Option<Instant>,
    node_snoop_empty_polls: u32,
    node_snoop_last_ui_log_at: Option<Instant>,
    node_snoop_active_trigger_id: u64,
    node_snoop_first_nonzero_at: Option<Instant>,
    node_snoop_first_ui_render_at: Option<Instant>,
    node_snoop_last_append_len: usize,
}

#[derive(Default)]
struct NodeSnoopShared {
    samples: Vec<f32>,
    last_snapshot_at: Option<Instant>,
    empty_polls: u32,
    poll_count: u64,
    active_trigger_id: u64,
    active_trigger_at: Option<Instant>,
    first_nonzero_trigger_id: u64,
    first_nonzero_at: Option<Instant>,
    first_nonzero_ui_render_trigger_id: u64,
    first_nonzero_ui_render_at: Option<Instant>,
    last_append_len: usize,
}

struct NodeSnoopWorker {
    should_stop: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<()>>,
}

impl NodeSnoopWorker {
    fn stop(&mut self) {
        self.should_stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

impl RuntimeDemoApp {
    fn shared_drag(
        ui: &mut egui::Ui,
        label: &str,
        shared: &Shared,
        range: std::ops::RangeInclusive<f64>,
    ) {
        ui.horizontal(|ui| {
            ui.label(label);
            let mut value = shared.value() as f64;
            if ui
                .add(egui::DragValue::new(&mut value).range(range).speed(0.01))
                .changed()
            {
                shared.set_value(value as f32);
            }
        });
    }

    fn trigger_once(&mut self) {
        self.trigger_seq = self.trigger_seq.saturating_add(1);
        let trigger_id = self.trigger_seq;
        let triggered_at = Instant::now();
        self.trigger_last_at = Some(triggered_at);

        {
            let mut shared = self.node_snoop_shared.lock();
            shared.active_trigger_id = trigger_id;
            shared.active_trigger_at = Some(triggered_at);
            shared.first_nonzero_trigger_id = 0;
            shared.first_nonzero_at = None;
            shared.first_nonzero_ui_render_trigger_id = 0;
            shared.first_nonzero_ui_render_at = None;
        }

        self.testbed.register_visual_trigger(trigger_id, triggered_at);
        log::debug!(
            "trigger_diag trigger_id={} trigger_age_ms={:.1}",
            trigger_id,
            triggered_at.elapsed().as_secs_f64() * 1000.0,
        );

        self.seq
            .lock()
            .push_relative(0.0, 1.0, Fade::Smooth, 0.0, 0.01, Box::new(impulse::<U1>()));
    }

    fn update_node_snoop_snapshot(&mut self) {
        let is_running = self.testbed.transport_state() == RuntimeTransportState::Running;
        self.sync_node_snoop_worker(is_running);

        if !is_running {
            self.node_snoop_samples.clear();
            self.node_snoop_last_snapshot_at = None;
            self.node_snoop_empty_polls = 0;
            self.node_snoop_last_ui_log_at = None;
            self.node_snoop_active_trigger_id = 0;
            self.node_snoop_first_nonzero_at = None;
            self.node_snoop_first_ui_render_at = None;
            self.node_snoop_last_append_len = 0;
            *self.node_snoop_shared.lock() = NodeSnoopShared::default();
            return;
        }

        {
            let now = Instant::now();
            let mut shared = self.node_snoop_shared.lock();
            if shared.first_nonzero_trigger_id == shared.active_trigger_id
                && shared.first_nonzero_trigger_id != 0
                && shared.first_nonzero_ui_render_trigger_id != shared.active_trigger_id
                && !shared.samples.is_empty()
            {
                shared.first_nonzero_ui_render_trigger_id = shared.active_trigger_id;
                shared.first_nonzero_ui_render_at = Some(now);
            }
            self.node_snoop_samples = shared.samples.clone();
            self.node_snoop_last_snapshot_at = shared.last_snapshot_at;
            self.node_snoop_empty_polls = shared.empty_polls;
            self.node_snoop_active_trigger_id = shared.active_trigger_id;
            self.node_snoop_first_nonzero_at = shared.first_nonzero_at;
            self.node_snoop_first_ui_render_at = shared.first_nonzero_ui_render_at;
            self.node_snoop_last_append_len = shared.last_append_len;
        }

        let now = Instant::now();
        let should_log = self.node_snoop_last_ui_log_at.is_none_or(|last| {
            now.duration_since(last) >= Duration::from_millis(SNAPSHOT_LOG_INTERVAL_MS)
        });
        if should_log {
            self.node_snoop_last_ui_log_at = Some(now);
            let snapshot_age_ms = self
                .node_snoop_last_snapshot_at
                .map(|timestamp| timestamp.elapsed().as_secs_f64() * 1000.0)
                .unwrap_or(-1.0);
            let marker_ms = match (
                self.trigger_last_at,
                self.node_snoop_first_nonzero_at,
                self.node_snoop_first_ui_render_at,
            ) {
                (Some(triggered_at), Some(first_nonzero_at), Some(first_render_at)) => format!(
                    "first_nonzero_ms={:.1} first_render_ms={:.1}",
                    first_nonzero_at.duration_since(triggered_at).as_secs_f64() * 1000.0,
                    first_render_at.duration_since(triggered_at).as_secs_f64() * 1000.0,
                ),
                _ => "first_nonzero_ms=n/a first_render_ms=n/a".to_string(),
            };
            log::debug!(
                "node_snoop_ui_diag transport=Running snapshot_age_ms={:.1} empty_polls={} samples={} last_append_len={} trigger_id={} {} worker_running={}",
                snapshot_age_ms,
                self.node_snoop_empty_polls,
                self.node_snoop_samples.len(),
                self.node_snoop_last_append_len,
                self.node_snoop_active_trigger_id,
                marker_ms,
                self.node_snoop_worker.is_some(),
            );
        }
    }

    fn sync_node_snoop_worker(&mut self, should_run: bool) {
        match (should_run, self.node_snoop_worker.is_some()) {
            (true, false) => self.start_node_snoop_worker(),
            (false, true) => self.stop_node_snoop_worker(),
            _ => {}
        }
    }

    fn start_node_snoop_worker(&mut self) {
        let should_stop = Arc::new(AtomicBool::new(false));
        let should_stop_for_thread = should_stop.clone();
        let node_rt = self.node_rt.clone();
        let shared = self.node_snoop_shared.clone();

        let spawn_result = thread::Builder::new()
            .name("node_adsr_snoop_worker".to_string())
            .spawn(move || {
                let poll_interval = Duration::from_millis(NODE_SNOOP_POLL_INTERVAL_MS);
                let mut last_log_at: Option<Instant> = None;

                while !should_stop_for_thread.load(Ordering::Relaxed) {
                    let mut samples = {
                        let mut node_rt = node_rt.lock();
                        node_rt
                            .snoop
                            .get()
                            .into_iter()
                            .flat_map(|buff| (0..buff.len()).map(|i| buff.at(i)).collect::<Vec<_>>())
                            .collect::<Vec<f32>>()
                    };

                    let now = Instant::now();
                    let mut shared = shared.lock();
                    shared.poll_count = shared.poll_count.saturating_add(1);

                    if samples.is_empty() {
                        shared.empty_polls = shared.empty_polls.saturating_add(1);
                    } else {
                        shared.last_append_len = samples.len();
                        shared.samples.extend(samples.drain(..));
                        if shared.samples.len() > NODE_SNOOP_WINDOW_SIZE {
                            let trim = shared.samples.len() - NODE_SNOOP_WINDOW_SIZE;
                            shared.samples.drain(..trim);
                        }

                        if shared.active_trigger_id != 0
                            && shared.first_nonzero_trigger_id != shared.active_trigger_id
                            && shared
                                .samples
                                .iter()
                                .copied()
                                .any(|sample| sample.abs() > NODE_SNOOP_NONZERO_EPSILON)
                        {
                            shared.first_nonzero_trigger_id = shared.active_trigger_id;
                            shared.first_nonzero_at = Some(now);
                        }

                        shared.last_snapshot_at = Some(now);
                        shared.empty_polls = 0;
                    }

                    if samples.is_empty() {
                        shared.last_append_len = 0;
                    }

                    let should_log = last_log_at.is_none_or(|last| {
                        now.duration_since(last) >= Duration::from_millis(SNAPSHOT_LOG_INTERVAL_MS)
                    });
                    if should_log {
                        last_log_at = Some(now);
                        let snapshot_age_ms = shared
                            .last_snapshot_at
                            .map(|timestamp| timestamp.elapsed().as_secs_f64() * 1000.0)
                            .unwrap_or(-1.0);
                        log::debug!(
                            "node_snoop_worker_diag poll_interval_ms={} snapshot_age_ms={:.1} empty_polls={} samples={} poll_count={}",
                            NODE_SNOOP_POLL_INTERVAL_MS,
                            snapshot_age_ms,
                            shared.empty_polls,
                            shared.samples.len(),
                            shared.poll_count,
                        );
                    }

                    drop(shared);
                    thread::sleep(poll_interval);
                }

                log::debug!("node_snoop_worker_diag stopped=true");
            });

        match spawn_result {
            Ok(handle) => {
                self.node_snoop_worker = Some(NodeSnoopWorker {
                    should_stop,
                    join_handle: Some(handle),
                });
                log::debug!(
                    "node_snoop_worker_diag started=true poll_interval_ms={}",
                    NODE_SNOOP_POLL_INTERVAL_MS
                );
            }
            Err(err) => {
                log::error!("node_snoop_worker_diag started=false error={}", err);
            }
        }
    }

    fn stop_node_snoop_worker(&mut self) {
        if let Some(mut worker) = self.node_snoop_worker.take() {
            worker.stop();
            log::debug!("node_snoop_worker_diag stop_requested=true");
        }
    }

    fn render_node_snoop_visualization(&self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Node output (snoop)")
            .id_salt("node_adsr_snoop_section")
            .default_open(true)
            .show(ui, |ui| {
                let age_ms = self
                    .node_snoop_last_snapshot_at
                    .map(|timestamp| timestamp.elapsed().as_secs_f64() * 1000.0);
                if let Some(age_ms) = age_ms {
                    ui.small(format!(
                        "Snapshot age: {:.1} ms (empty polls: {})",
                        age_ms, self.node_snoop_empty_polls
                    ));
                } else {
                    ui.small(format!(
                        "Snapshot age: n/a (empty polls: {})",
                        self.node_snoop_empty_polls
                    ));
                }

                let marker_text = match (
                    self.trigger_last_at,
                    self.node_snoop_first_nonzero_at,
                    self.node_snoop_first_ui_render_at,
                ) {
                    (Some(triggered_at), Some(first_nonzero_at), Some(first_render_at)) => format!(
                        "Trigger #{} first nonzero: {:.1} ms, first render: {:.1} ms",
                        self.node_snoop_active_trigger_id,
                        first_nonzero_at.duration_since(triggered_at).as_secs_f64() * 1000.0,
                        first_render_at.duration_since(triggered_at).as_secs_f64() * 1000.0,
                    ),
                    _ => format!(
                        "Trigger #{} first nonzero/render: pending",
                        self.node_snoop_active_trigger_id
                    ),
                };

                show_buffer_line_chart(
                    ui,
                    "node_adsr_snoop_waveform",
                    &self.node_snoop_samples,
                    "No node output snapshot yet",
                    egui::Color32::from_rgb(255, 196, 92),
                );
                ui.small(marker_text);
            });
    }
}

impl eframe::App for RuntimeDemoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.update_node_snoop_snapshot();

        show_scrolled_left_panel_inside(ui, "runtime_controls", 360.0, None, |ui| {
            self.node.lock().ui_mut(ui);

            ui.separator();
            ui.heading("Node shared controls");
            {
                let node_rt = self.node_rt.lock();
                Self::shared_drag(
                    ui,
                    "path_spread_coeff",
                    &node_rt.path_spread_coeff,
                    -1.0..=1.0,
                );
                Self::shared_drag(
                    ui,
                    "mode_spacing_coeff",
                    &node_rt.mode_spacing_coeff,
                    -1.0..=1.0,
                );
            }

            ui.separator();
            ui.heading("ADSR shared controls");
            Self::shared_drag(ui, "dur_0", &self.adsr_dur_0, 0.001..=4.0);
            Self::shared_drag(ui, "weight_0", &self.adsr_weight_0, -2.0..=2.0);
            Self::shared_drag(ui, "dur_1", &self.adsr_dur_1, 0.001..=4.0);
            Self::shared_drag(ui, "weight_1", &self.adsr_weight_1, -2.0..=2.0);
            Self::shared_drag(ui, "dur_2", &self.adsr_dur_2, 0.001..=4.0);
            Self::shared_drag(ui, "weight_2", &self.adsr_weight_2, -2.0..=2.0);

            self.testbed.render_controls(ui);

            ui.separator();
            let response = ui.add(egui::Button::new("Trigger ADSR (click or Space)"));
            let pointer_down = response.is_pointer_button_down_on();
            let key_down = ui.ctx().input(|input| input.key_down(egui::Key::Space));
            let pressed_now = pointer_down || key_down;
            if pressed_now && !self.trigger_prev_down {
                self.trigger_once();
            }
            self.trigger_prev_down = pressed_now;
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.render_node_snoop_visualization(ui);
            ui.separator();
            self.testbed.render_diagnostics(ui);
        });
    }
}

impl Drop for RuntimeDemoApp {
    fn drop(&mut self) {
        self.stop_node_snoop_worker();
    }
}
