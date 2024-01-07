use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use crux_core::render::Render;
use crux_core::App;
use crux_kv::{KeyValue, KeyValueOutput};
use crux_macros::Effect;
use hecs::World;
use mint::Point2;
use serde::{Deserialize, Serialize};

use crate::{geometry::{Line, Rect}, instrument::{self, layout::MenuPosition}, Navigate, Play};

use self::chart::{Chart, FFTChartEntry, Pair};

mod chart;

pub const MIN_F: f32 = 100.0;
pub const MAX_F: f32 = 12_000.0;

#[derive(Default)]
pub struct Tuner;

#[derive(Default, Clone, PartialEq, Eq)]
pub enum State {
    #[default]
    None,
    SetupInProgress,
    SetupComplete,
    Capturing,
}

#[derive(Default, Clone)]
pub struct Model {
    pub world: Arc<Mutex<World>>,
    pub chart: Option<Chart>,
    pub persisted: bool,
    pub config: instrument::Config,
    pub tuning: Option<Vec<(usize, f32, f32)>>,
    pub state: State,
    pub pressed_buttons: HashSet<usize>,
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
    SetTuning(Option<Vec<(usize, f32, f32)>>),
    TuningKV(KeyValueOutput),
    SetFreqAmpXYPos(usize, f64, f64),
    ButtonPress(usize, bool),
    ButtonReleaseAll,
    SetConfig(instrument::Config),
    Activate(bool),
    FftData(Vec<(f32, f32)>),
    PlayOpSuccess(bool),
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
        log::trace!("tuner ev: {event:?}");

        match event {
            TunerEV::CheckHasTuning => {
                caps.key_value.read("tuning", TunerEV::TuningKV);
            }
            TunerEV::SetConfig(config) => {
                {
                    let mut world = model.world.lock().expect("world lock");
                    if let Some(old) = model.chart.take() {
                        old.delete(&mut world);
                    }
                    model.config = config;
                    model.chart = Some(Chart::new(&mut world, &model.config));

                    model.menu_position = MenuPosition::TopLeft(
                        Rect::size(128.0, 82.0)
                            .offset_left(-model.config.safe_area[0])
                            .offset_top(-model.config.safe_area[1]),
                    );
                }

                self.update(TunerEV::SetTuning(model.tuning.clone()), model, caps);

                caps.render.render();
            }
            TunerEV::Activate(start) => {
                if model.state == State::SetupComplete {
                    if start {
                        caps.play.play(TunerEV::PlayOpSuccess);
                        model.state = State::Capturing;
                    } else {
                        caps.play.stop_capture_fft(TunerEV::PlayOpSuccess);
                        model.state = State::SetupComplete;

                        let pairs = self.get_pairs(model);
                        let values = pairs
                            .iter()
                            .map(|p| (p.f_n, p.value.unwrap_or((0.0, 0.0))))
                            .collect::<Vec<_>>();
                        caps.key_value.write(
                            "tuning",
                            bincode::serialize(&values).expect("serialize tuning"),
                            TunerEV::TuningKV,
                        )
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
                    model.state = State::None;
                    self.update(TunerEV::PlayOpSuccess(false), model, caps);
                } else {
                    model.state = State::SetupComplete;
                    self.update(TunerEV::Activate(true), model, caps);
                }
            }
            TunerEV::PlayOpSuccess(success) => {
                if !success {
                    log::error!("tuner play op failed");
                    caps.navigate.to(crate::Activity::Intro)
                } else if model.state == State::Capturing {
                    caps.play.capture_fft(TunerEV::FftData);
                } else {
                    caps.play.pause();
                    log::info!("play operation completed with success");
                }
            }
            TunerEV::SetTuning(value) => {
                {
                    model.persisted = value.is_some();
                    if let Some(values) = value.as_ref() {
                        if let Some(chart) = model.chart.as_ref() {
                            let mut world = model.world.lock().expect("world lock");
                            chart.update_pairs_from_values(&mut world, values, &model.config)
                        }
                    }
                    model.tuning = value;
                }

                caps.render.render();
            }
            TunerEV::ButtonPress(f_n, pressed) => {
                if pressed {
                    _ = model.pressed_buttons.insert(f_n);
                } else {
                    _ = model.pressed_buttons.remove(&f_n);
                }

                caps.render.render();
            }
            TunerEV::ButtonReleaseAll => {
                model.pressed_buttons.clear();
                caps.render.render();
            }
            TunerEV::SetFreqAmpXYPos(f_n, value_x, value_y) => {
                if model.pressed_buttons.contains(&f_n) {
                    let mut world = model.world.lock().expect("world lock");
                    let chart = model.chart.as_mut().expect("chart");
                    if value_y < (model.config.height - model.config.safe_area[3])
                        && value_y > model.config.safe_area[1]
                    {
                        if f_n == 0
                            || world.query::<&Pair>().into_iter().all(|(_, p)| {
                                if p.f_n == f_n + 1 {
                                    (p.rect.center().x + model.config.button_size / 2.0) < value_x
                                } else if p.f_n == f_n - 1 {
                                    (p.rect.center().x - model.config.button_size / 2.0) > value_x
                                } else {
                                    true
                                }
                            })
                        {
                            let (x, y, f_n) = {
                                let query = world.query_mut::<&mut Pair>();
                                let (_, pair) = query
                                    .into_iter()
                                    .find(|(_, p)| p.f_n == f_n)
                                    .expect("pair for f_n");
                                pair.rect.move_x(value_x);
                                (value_x, pair.rect.center().y, pair.f_n)
                            };

                            chart.update_value_from_pos(&mut world, f_n, (&x, &y), &model.config)
                        }

                        let (x, y, f_n) = {
                            let query = world.query_mut::<&mut Pair>();
                            let (_, pair) = query
                                .into_iter()
                                .find(|(_, p)| p.f_n == f_n)
                                .expect("pair for f_n");
                            pair.rect.move_y(value_y);
                            (pair.rect.center().x, value_y, pair.f_n)
                        };
                        chart.update_value_from_pos(&mut world, f_n, (&x, &y), &model.config)
                    }

                    caps.render.render();
                }
            }
            TunerEV::TuningKV(kv) => match kv {
                KeyValueOutput::Read(value) => {
                    if let Some(v) = value {
                        match bincode::deserialize::<Vec<(usize, f32, f32)>>(v.as_slice()) {
                            Ok(v) => {
                                self.update(TunerEV::SetTuning(Some(v)), model, caps);
                            }
                            Err(e) => {
                                log::error!("{e:?}");
                                self.update(TunerEV::SetTuning(None), model, caps);
                            }
                        }
                    } else {
                        log::info!("no tuner data");
                        self.update(TunerEV::SetTuning(None), model, caps);
                    };
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
    fn get_pairs(&self, model: &Model) -> Vec<Pair> {
        let world = model.world.lock().expect("world lock");
        model
            .chart
            .as_ref()
            .map(|ch| {
                ch.pairs
                    .iter()
                    .map(|e| *world.get::<&Pair>(*e).expect("Pair for entity"))
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
