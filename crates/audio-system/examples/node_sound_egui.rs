use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use audio_system::egui_testbed::{RuntimeTestbed, RuntimeTransportState};
use common::egui_helpers::{
    run_native_app, show_buffer_line_chart, show_scrolled_left_panel_inside,
};
use eframe::egui;
use enum2egui::GuiInspect;
use fundsp::prelude::*;
use parking_lot::Mutex;

const WINDOW_TITLE: &str = "Node demo";
const WINDOW_SIZE: [f32; 2] = [1180.0, 760.0];
const NODE_SNOOP_WINDOW_SIZE: usize = 256;
const SNAPSHOT_LOG_INTERVAL_MS: u64 = 1000;
const NODE_SNOOP_POLL_INTERVAL_MS: u64 = 5;

fn main() -> eframe::Result<()> {
    run_native_app(
        WINDOW_TITLE,
        WINDOW_SIZE,
        Some("audio_system=debug,node_sound_egui=debug"),
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
                mode_decay_s: 1.2,
            };
            let mut app = RuntimeDemoApp {
                testbed: RuntimeTestbed::default(),
                node: Arc::new(Mutex::new(node_cfg)),
                node_rt: Arc::new(Mutex::new(audio_system::system::node::Node::new(node_cfg))),
                xct_1: true,
                xct_2: false,
                xct_3: false,
                sh_xct_1: Arc::new(Shared::new(0.0)),
                sh_xct_2: Arc::new(Shared::new(0.0)),
                sh_xct_3: Arc::new(Shared::new(0.0)),
                node_snoop_shared: Arc::new(Mutex::new(NodeSnoopShared::default())),
                node_snoop_worker: None,
                node_snoop_samples: Vec::new(),
                node_snoop_last_snapshot_at: None,
                node_snoop_empty_polls: 0,
                node_snoop_last_ui_log_at: None,
            };

            let node_rt = app.node_rt.clone();
            let node_cfg = app.node.clone();
            let sh_xct_1 = app.sh_xct_1.clone();
            let sh_xct_2 = app.sh_xct_2.clone();
            let sh_xct_3 = app.sh_xct_3.clone();

            app.testbed.set_startup_network_builder(move |sample_rate| {
                *node_rt.lock() = audio_system::system::node::Node::new(*node_cfg.lock());

                let mut net = Net::new(1, 2);
                let input_id = net.push(Box::new(sink()));
                let node_id = net.push(Box::new(node_rt.lock().backend()));
                let split_id = net.push(Box::new(split::<U2>()));

                let backend_id = net.push(Box::new(
                    var(&sh_xct_1) >> dcblock_hz::<f32>(50.0)
                        | var(&sh_xct_2) >> dcblock_hz::<f32>(50.0)
                        | var(&sh_xct_3) >> dcblock_hz::<f32>(50.0),
                ));

                net.pipe_all(backend_id, node_id);
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
    node_rt: Arc<Mutex<audio_system::system::node::Node<f32>>>,
    xct_1: bool,
    xct_2: bool,
    xct_3: bool,
    sh_xct_1: Arc<Shared>,
    sh_xct_2: Arc<Shared>,
    sh_xct_3: Arc<Shared>,
    node_snoop_shared: Arc<Mutex<NodeSnoopShared>>,
    node_snoop_worker: Option<NodeSnoopWorker>,
    node_snoop_samples: Vec<f32>,
    node_snoop_last_snapshot_at: Option<Instant>,
    node_snoop_empty_polls: u32,
    node_snoop_last_ui_log_at: Option<Instant>,
}

#[derive(Default)]
struct NodeSnoopShared {
    samples: Vec<f32>,
    last_snapshot_at: Option<Instant>,
    empty_polls: u32,
    poll_count: u64,
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
    fn update_node_snoop_snapshot(&mut self) {
        let is_running = self.testbed.transport_state() == RuntimeTransportState::Running;
        self.sync_node_snoop_worker(is_running);

        if !is_running {
            self.node_snoop_samples.clear();
            self.node_snoop_last_snapshot_at = None;
            self.node_snoop_empty_polls = 0;
            self.node_snoop_last_ui_log_at = None;
            *self.node_snoop_shared.lock() = NodeSnoopShared::default();
            return;
        }

        {
            let shared = self.node_snoop_shared.lock();
            self.node_snoop_samples = shared.samples.clone();
            self.node_snoop_last_snapshot_at = shared.last_snapshot_at;
            self.node_snoop_empty_polls = shared.empty_polls;
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
            log::debug!(
                "node_snoop_ui_diag transport=Running snapshot_age_ms={:.1} empty_polls={} samples={} worker_running={}",
                snapshot_age_ms,
                self.node_snoop_empty_polls,
                self.node_snoop_samples.len(),
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
            .name("node_sound_snoop_worker".to_string())
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
                        if samples.len() > NODE_SNOOP_WINDOW_SIZE {
                            let trim = samples.len() - NODE_SNOOP_WINDOW_SIZE;
                            samples.drain(..trim);
                        }

                        shared.samples = samples;
                        shared.last_snapshot_at = Some(now);
                        shared.empty_polls = 0;
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
            .id_salt("node_sound_snoop_section")
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

                show_buffer_line_chart(
                    ui,
                    "node_sound_snoop_waveform",
                    &self.node_snoop_samples,
                    "No node output snapshot yet",
                    egui::Color32::from_rgb(255, 196, 92),
                );
            });
    }
}

impl eframe::App for RuntimeDemoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.update_node_snoop_snapshot();

        show_scrolled_left_panel_inside(ui, "runtime_controls", 360.0, None, |ui| {
            self.node.lock().ui_mut(ui);
            let node_rt = self.node_rt.clone();
            ui.add(
                egui::widgets::DragValue::from_get_set(move |val| {
                    let lock = node_rt.lock();
                    let shared = &lock.path_spread_coeff;
                    if let Some(val) = val {
                        shared.set_value(val as f32);
                    }
                    shared.value() as f64
                })
                .range(-1.0..=1.0),
            );
            let node_rt = self.node_rt.clone();
            ui.add(
                egui::widgets::DragValue::from_get_set(move |val| {
                    let lock = node_rt.lock();
                    let shared = &lock.mode_spacing_coeff;
                    if let Some(val) = val {
                        shared.set_value(val as f32);
                    }
                    shared.value() as f64
                })
                .range(-1.0..=1.0),
            );
            self.testbed.render_controls(ui);
            ui.separator();
            ui.checkbox(&mut self.xct_1, "Excite first path");
            ui.checkbox(&mut self.xct_2, "Excite second path");
            ui.checkbox(&mut self.xct_3, "Excite third path");
            let response = ui.add(egui::Button::new("Excite (hold mouse or Space)"));
            let pointer_held = response.is_pointer_button_down_on();
            let key_held = ui.ctx().input(|input| input.key_down(egui::Key::Space));

            if pointer_held || key_held {
                if self.xct_1 {
                    self.sh_xct_1.set_value(1.0);
                }
                if self.xct_2 {
                    self.sh_xct_2.set_value(1.0);
                }
                if self.xct_3 {
                    self.sh_xct_3.set_value(1.0);
                }
            } else {
                self.sh_xct_1.set_value(0.0);
                self.sh_xct_2.set_value(0.0);
                self.sh_xct_3.set_value(0.0);
            }
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
