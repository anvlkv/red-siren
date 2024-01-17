use std::sync::{Arc, Mutex};

use crux_core::render::Render;
use crux_core::App;
use crux_kv::{KeyValue, KeyValueOutput};
use crux_macros::Effect;
use hecs::World;
use mint::Point2;
use serde::{Deserialize, Serialize};

use crate::{
    geometry::{Line, Rect},
    instrument::{self, layout::MenuPosition},
    Navigate, Play,
};


mod chart;
pub use self::chart::{Chart, FFTChartEntry, Pair, TriggerState};

pub const MIN_F: f32 = 0.06;
pub const MAX_F: f32 = 6_000.0;

pub type TuningValue = (usize, f32, f32);

#[derive(Default)]
pub struct Tuner;

#[derive(Default, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Copy)]
pub enum State {
    #[default]
    None,
    SetupInProgress,
    SetupComplete,
    Capturing,
    Done,
}

#[derive(Default, Clone)]
pub struct Model {
    pub world: Arc<Mutex<World>>,
    pub chart: Option<Chart>,
    pub persisted: bool,
    pub config: instrument::Config,
    pub tuning: Option<Vec<TuningValue>>,
    pub state: State,
    pub menu_position: MenuPosition,
}

impl Model {
    pub fn new(world: Arc<Mutex<World>>) -> Self {
        Self {
            world,
            ..Default::default()
        }
    }
}

#[derive(Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct TunerVM {
    pub pairs: Vec<Pair>,
    pub line: Line,
    pub needs_tuning: bool,
    pub range: f64,
    pub fft: Vec<Point2<f64>>,
    pub fft_max: Vec<Point2<f64>>,
    pub menu_position: MenuPosition,
}

impl Eq for TunerVM {}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum TunerEV {
    CheckHasTuning,
    TuningKV(KeyValueOutput),
    MovementXY((f64, f64), i32),
    ActivationXY((f64, f64), i32),
    DeactivationXY(i32),
    SetConfig(instrument::Config),
    Activate(bool),
    FftData(Vec<(f32, f32)>),
    PlayOpStartProcessing(bool),
    PlayOpStartCapturing(bool),
    PlayOpStopProcessing(bool),
    PlayOpStopCapturing(bool),
    PlayOpPermission(bool),
    PlayOpInstall(bool),
}

impl Eq for TunerEV {}

#[cfg_attr(feature = "typegen", derive(crux_macros::Export))]
#[derive(Effect)]
#[effect(app = "Tuner")]
pub struct TunerCapabilities {
    pub render: Render<TunerEV>,
    pub key_value: KeyValue<TunerEV>,
    pub play: Play<TunerEV>,
    pub navigate: Navigate<TunerEV>,
}

impl App for Tuner {
    type Event = TunerEV;

    type Model = Model;

    type ViewModel = TunerVM;

    type Capabilities = TunerCapabilities;

    fn update(&self, event: Self::Event, model: &mut Self::Model, caps: &Self::Capabilities) {
        // log::trace!("tuner ev: {event:?}");

        match event {
            TunerEV::CheckHasTuning => {
                // caps.key_value.read("tuning", TunerEV::TuningKV);
            }
            TunerEV::SetConfig(config) => {
                {
                    let mut world = model.world.lock().expect("world lock");
                    if let Some(old) = model.chart.take() {
                        old.delete(&mut world);
                    }
                    model.config = config;
                    model.chart = Some(Chart::new(&mut world, &model.config));
                }

                self.update_pairs_from_values(model);

                model.menu_position = MenuPosition::TopLeft(
                    Rect::size(128.0, 82.0)
                        .offset_left(-model.config.safe_area[0])
                        .offset_top(-model.config.safe_area[1]),
                );

                caps.render.render();
            }
            TunerEV::Activate(start) => {
                if model.state >= State::SetupComplete {
                    if start {
                        caps.play.play(TunerEV::PlayOpStartProcessing);
                    } else {
                        let pairs = self.get_pairs(model);
                        let values = pairs
                            .iter()
                            .map(|p| {
                                let val = p.value.unwrap_or((0.0, 0.0));
                                (p.f_n, val.0, val.1)
                            })
                            .collect::<Vec<TuningValue>>();
                        model.tuning = Some(values.clone());
                        caps.play.stop_capture_fft(TunerEV::PlayOpStopCapturing);
                        // caps.key_value.write(
                        //     "tuning",
                        //     bincode::serialize(&values).expect("serialize tuning"),
                        //     TunerEV::TuningKV,
                        // );
                        log::info!("tuning complete and stored");
                    }
                } else if model.state != State::SetupInProgress {
                    caps.play.permissions(TunerEV::PlayOpPermission);
                    model.state = State::SetupInProgress;
                }
            }
            TunerEV::FftData(data) => {
                {
                    let mut world = model.world.lock().expect("world lock");
                    model.chart.as_mut().expect("chart").set_fft_data(
                        &mut world,
                        data,
                        &model.config,
                    );
                }
                caps.render.render();
            }
            TunerEV::PlayOpPermission(grant) => {
                if grant {
                    caps.play.install_au(TunerEV::PlayOpInstall);
                } else {
                    model.state = State::None;
                    caps.navigate.to(crate::Activity::Intro);
                }
            }
            TunerEV::PlayOpInstall(success) => {
                if !success {
                    log::error!("tuner play op failed");
                    caps.navigate.to(crate::Activity::Intro);
                    model.state = State::None;
                } else {
                    model.state = State::SetupComplete;
                    self.update(TunerEV::Activate(true), model, caps);
                }
            }
            TunerEV::PlayOpStartProcessing(success) => {
                if !success {
                    log::error!("tuner play op failed");
                    caps.navigate.to(crate::Activity::Intro);
                    model.state = State::None;
                } else {
                    caps.play.capture_fft(TunerEV::PlayOpStartCapturing)
                }
            }
            TunerEV::PlayOpStartCapturing(success) => {
                if !success {
                    log::error!("tuner play op failed");
                    caps.navigate.to(crate::Activity::Intro);
                    model.state = State::None;
                } else {
                    model.state = State::Capturing;
                }
            }
            TunerEV::PlayOpStopProcessing(success) => {
                if !success {
                    log::error!("tuner play op failed");
                    caps.navigate.to(crate::Activity::Intro);
                    model.state = State::None;
                } else {
                    log::info!("done capturing");
                }
            }
            TunerEV::PlayOpStopCapturing(success) => {
                if !success {
                    log::error!("tuner play op failed");
                    caps.navigate.to(crate::Activity::Intro);
                    model.state = State::None;
                } else {
                    caps.play.pause(TunerEV::PlayOpStopProcessing)
                }
            }
            TunerEV::ActivationXY((x, y), id) => {
                {
                    let world = model.world.lock().expect("world lock");

                    if let Some(mut pair) = model
                        .chart
                        .as_ref()
                        .map(|ch| {
                            ch.pairs
                                .iter()
                                .map(|e| world.get::<&mut Pair>(*e).expect("Pair for entity"))
                                .find(|p| p.rect.contains(Point2 { x, y }))
                        })
                        .flatten()
                    {
                        pair.finger = Some(id);
                    };
                }
                caps.render.render();
            }
            TunerEV::DeactivationXY(id) => {
                {
                    let world = model.world.lock().expect("world lock");

                    if let Some(mut pair) = model
                        .chart
                        .as_ref()
                        .map(|ch| {
                            ch.pairs
                                .iter()
                                .map(|e| world.get::<&mut Pair>(*e).expect("Pair for entity"))
                                .find(|p| p.finger == Some(id))
                        })
                        .flatten()
                    {
                        pair.finger = None;
                    };
                }
                caps.render.render();
            }
            TunerEV::MovementXY((x, y), id) => {
                {
                    let mut world = model.world.lock().expect("world lock");
                    let f_n = model
                        .chart
                        .as_ref()
                        .map(|ch| {
                            ch.pairs
                                .iter()
                                .map(|e| world.get::<&Pair>(*e).expect("Pair for entity"))
                                .find_map(|p| {
                                    if p.finger == Some(id) {
                                        Some(p.f_n)
                                    } else {
                                        None
                                    }
                                })
                        })
                        .flatten();

                    if let Some(f_n) = f_n {
                        model.chart.as_ref().unwrap().update_value_from_pos(
                            &mut world,
                            f_n,
                            (&x, &y),
                            &model.config,
                        );
                    };
                }
                caps.render.render();
            }
            TunerEV::TuningKV(kv) => match kv {
                KeyValueOutput::Read(value) => {
                    model.persisted = value.is_some();
                    model.tuning = value
                        .map(|d| bincode::deserialize::<Vec<TuningValue>>(d.as_slice()).ok())
                        .flatten();
                    self.update_pairs_from_values(model);
                }
                KeyValueOutput::Write(success) => model.persisted = success,
            },
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        let (fft, fft_max) = self.get_fft(model).into_iter().unzip();
        TunerVM {
            pairs: self.get_pairs(model),
            needs_tuning: !self.is_tuned(model),
            line: model.chart.as_ref().map(|ch| ch.line).unwrap_or_default(),
            range: model.config.height,
            fft,
            fft_max,
            menu_position: model.menu_position.clone(),
        }
    }
}

impl Tuner {
    fn update_pairs_from_values(&self, model: &mut Model) {
        if let Some((chart, values)) = model.chart.as_mut().zip(model.tuning.as_ref()) {
            let mut world = model.world.lock().expect("world lock");
            chart.update_pairs_from_values(&mut world, values, &model.config);
            log::info!("tuning data applied");
        } else {
            log::warn!("no chart or tuning values");
        }
    }

    fn get_pairs(&self, model: &Model) -> Vec<Pair> {
        let world = model.world.lock().expect("world lock");
        model
            .chart
            .as_ref()
            .map(|ch| {
                ch.pairs
                    .iter()
                    .map(|e| *world.get::<&Pair>(*e).expect("Pair for entity"))
                    .map(|p| {
                        log::debug!("vm pair: {:?}", p.triggered);
                        p
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn get_fft(&self, model: &Model) -> Vec<(Point2<f64>, Point2<f64>)> {
        model
            .chart
            .as_ref()
            .map(|ch| {
                let world = model.world.lock().expect("world lock");

                ch.fft_values
                    .iter()
                    .filter_map(|e| world.get::<&FFTChartEntry>(*e).ok())
                    .map(|e| e.pt_max.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    pub fn is_tuned(&self, model: &Model) -> bool {
        model
            .chart
            .as_ref()
            .map(|ch| {
                let world = model.world.lock().expect("world lock");
                ch.pairs
                    .iter()
                    .filter_map(|e| world.get::<&Pair>(*e).ok())
                    .filter(|p| p.value.is_some())
                    .count()
                    >= model.config.n_buttons
            })
            .unwrap_or_default()
    }
}
