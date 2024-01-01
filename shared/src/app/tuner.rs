use std::sync::{Arc, Mutex};

use crux_core::render::Render;
use crux_core::App;
use crux_kv::{KeyValue, KeyValueOutput};
use crux_macros::Effect;
use hecs::{Entity, World};
use serde::{Deserialize, Serialize};

use crate::{geometry::Line, instrument, Play, Navigate};

use self::chart::{Chart, Pair};

mod chart;

#[derive(Default)]
pub struct Tuner;

#[derive(Default, Clone)]
pub struct Model {
    pub world: Arc<Mutex<World>>,
    pub pairs: Vec<Entity>,
    pub chart: Option<Entity>,
    pub persisted: bool,
    pub config: instrument::Config,
    pub tuning: Option<Vec<(usize, f32)>>,
    pub setup_complete: bool,

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
    pub fft: Vec<(f32, f32)>
}

impl Eq for TunerVM {}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum TunerEV {
    CheckHasTuning,
    SetTuning(Option<Vec<(usize, f32)>>),
    TuningKV(KeyValueOutput),
    SetTuningValue(usize, f32),
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
                    model.config = config;
                    model.chart = Some(Chart::spawn(&mut world, &model.config));
                    let mut query = world.query::<&Pair>();
                    let mut pairs = query.iter().collect::<Vec<_>>();
                    pairs.sort_by(|a, b| a.1.f_n.cmp(&b.1.f_n));
                    model.pairs = pairs.into_iter().map(|(e, _)| e).collect();
                }

                self.update(TunerEV::SetTuning(model.tuning.clone()), model, caps);

                caps.render.render();
            }
            TunerEV::Activate(start) => {
                if model.setup_complete {
                    if start {
                        caps.play.capture_fft(TunerEV::FftData);
                        caps.play.play(TunerEV::PlayOpSuccess);
                    }
                    else {
                        caps.play.stop_capture_fft(TunerEV::PlayOpSuccess);
                        caps.play.pause(TunerEV::PlayOpSuccess);
                    }
                }
                else {
                    caps.play.permissions(TunerEV::PlayOpPermission);
                }
            }
            TunerEV::FftData(data) => {
                {
                    let mut world = model.world.lock().expect("world lock");
                    
                    let query = world.query_mut::<&mut Chart>();
                    let (_, chart) = query.into_iter().next().expect("chart entity");
                    chart.set_fft_data(data);
                }
                caps.render.render();
            }
            TunerEV::PlayOpPermission(grant) => {
                if grant {
                    caps.play.install_au(TunerEV::PlayOpInstall)
                } else {
                    caps.navigate.to(crate::Activity::Intro)
                }
            }
            TunerEV::PlayOpInstall(success) => {
                if !success {
                    self.update(TunerEV::PlayOpSuccess(false), model, caps);
                } else {
                    model.setup_complete = true;
                    self.update(TunerEV::Activate(true), model, caps);
                }
            }
            TunerEV::PlayOpSuccess(success) => {
                if !success {
                    log::error!("tuner play op failed")
                }
            }
            TunerEV::SetTuning(value) => {
                {
                    let mut world = model.world.lock().expect("world lock");
                    model.persisted = value.is_some();
                    if let Some(value) = value.as_ref() {
                        for (_, pair) in world.query_mut::<&mut Pair>().into_iter() {
                            pair.update_from_values(value.as_slice(), &model.config);
                        }
                    }
                    model.tuning = value;
                }

                caps.render.render();
            }
            TunerEV::SetTuningValue(f_n, value) => {
                {
                    let mut world = model.world.lock().expect("world lock");
                    let query = world.query_mut::<&mut Pair>();
                    let (_, pair) = query
                        .into_iter()
                        .filter(|(_, p)| p.f_n == f_n)
                        .next()
                        .expect("pair for f_n");
                    pair.set_value(value, &model.config);
                }

                caps.render.render();

                let pairs = self.get_pairs(&model);
                let values = pairs.iter().map(|p| (p.f_n, p.value)).collect::<Vec<_>>();
                caps.key_value.write(
                    "tuning",
                    bincode::serialize(&values).expect("serialize tuning"),
                    TunerEV::TuningKV,
                )
            }
            TunerEV::TuningKV(kv) => match kv {
                KeyValueOutput::Read(value) => {
                    if let Some(v) = value {
                        match bincode::deserialize::<Vec<(usize, f32)>>(v.as_slice()) {
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
        TunerVM {
            pairs: self.get_pairs(model),
            needs_tuning: self.needs_tuning(model),
            line: self.get_line(model),
            range: model.config.height,
            fft: self.get_fft(model)
        }
    }
}

impl Tuner {
    fn needs_tuning(&self, model: &Model) -> bool {
        model.pairs.len() == 0
            || model.pairs.len() < {
                let world = model.world.lock().expect("world lock");
                let len = world.query::<&instrument::Node>().iter().len();
                len
            }
    }
    fn get_pairs(&self, model: &Model) -> Vec<Pair> {
        let world = model.world.lock().expect("world lock");
        model
            .pairs
            .iter()
            .map(|e| *world.get::<&Pair>(*e).expect("Pair for entity"))
            .collect()
    }
    fn get_line(&self, model: &Model) -> Line {
        let world = model.world.lock().expect("world lock");
        let mut query = world.query::<&Chart>();
        if let Some((_, chart)) = query.iter().next() {
            chart.line.clone()
        } else {
            Line::default()
        }
    }
    fn get_fft(&self, model: &Model) -> Vec<(f32, f32)> {
        let world = model.world.lock().expect("world lock");
        let mut query = world.query::<&Chart>();
        if let Some((_, chart)) = query.iter().next() {
            chart.fft_values.clone()
        } else {
            vec![]
        }
    }
}
