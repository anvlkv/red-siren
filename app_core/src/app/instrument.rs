use std::{
    cmp::Ordering,
    sync::{Arc, Mutex},
};

use crux_core::render::Render;
use crux_core::App;
use crux_macros::Effect;
use hecs::{Entity, World};
use mint::Point2;
use serde::{Deserialize, Serialize};

pub use config::Config;
pub use layout::{Layout, LayoutRoot};
use node::spawn_all_nodes;
pub use node::Node;

use crate::{play::Play, tuner::TuningValue, Navigate};

use self::string::OutboundString;

pub mod config;
pub mod keyboard;
pub mod layout;
pub mod node;
pub mod string;

#[derive(Default)]
pub struct Instrument;

#[derive(Default)]
pub struct Model {
    pub config: Config,
    pub world: Arc<Mutex<World>>,
    pub inbound: Option<Entity>,
    pub outbound: Option<Entity>,
    pub keyboard: Option<Entity>,
    pub root: Option<Entity>,
    pub nodes: Vec<Entity>,
    pub playing: bool,
    pub layout: Option<Layout>,
    pub setup_complete: bool,
    pub configured: bool,
    pub tuning: Vec<TuningValue>,
    pub snooped: Vec<f32>,
}

impl Model {
    #[must_use]
    pub fn new(world: Arc<Mutex<World>>) -> Self {
        Self {
            world,
            ..Default::default()
        }
    }
}

#[derive(Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct InstrumentVM {
    pub config: Config,
    pub nodes: Vec<Node>,
    pub playing: bool,
    pub layout: Layout,
    pub data_out: Vec<Point2<f64>>,
}

impl Eq for InstrumentVM {}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum PlaybackEV {
    Play(bool),
    Error,
}

impl Eq for PlaybackEV {}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum InstrumentEV {
    None,
    CreateWithConfig(Config),
    Playback(PlaybackEV),
    PlayOpPermission(bool),
    PlayOpInstall(bool),
    PlayOpConfigure(bool),
    PlayOpPlay(bool),
    PlayOpPause(bool),
    SnoopData(Vec<f32>),
    NodeSnoopData(Vec<(usize, Vec<f32>)>),
    RequestSnoops,
}

impl Eq for InstrumentEV {}

#[cfg_attr(feature = "typegen", derive(crux_macros::Export))]
#[derive(Effect)]
#[effect(app = "Instrument")]
pub struct InstrumentCapabilities {
    pub render: Render<InstrumentEV>,
    pub play: Play<InstrumentEV>,
    pub navigate: Navigate<InstrumentEV>,
}

impl App for Instrument {
    type Event = InstrumentEV;

    type Model = Model;

    type ViewModel = InstrumentVM;

    type Capabilities = InstrumentCapabilities;

    fn update(&self, event: Self::Event, model: &mut Self::Model, caps: &Self::Capabilities) {
        match event {
            InstrumentEV::CreateWithConfig(config) => {
                model.config = config.clone();
                {
                    let mut world = model.world.lock().expect("world lock");

                    let inbound = string::InboundString::spawn(&mut world, &config);
                    let outbound = string::OutboundString::spawn(&mut world, &config);
                    let keyboard = keyboard::Keyboard::spawn(&mut world, &config);

                    let root = layout::LayoutRoot::spawn(&mut world, inbound, outbound, keyboard);

                    let layout = Layout::new(&world, &root, &config).expect("Layout failed");
                    _ = model.layout.insert(layout);

                    _ = model.root.insert(root);
                    _ = model.inbound.insert(inbound);
                    _ = model.outbound.insert(outbound);
                    _ = model.keyboard.insert(keyboard);

                    model.nodes = spawn_all_nodes(&mut world);
                }

                if model.playing {
                    if model.nodes.len() > model.tuning.len() {
                        caps.navigate.to(crate::Activity::Tune)
                    } else {
                        let nodes = self.get_nodes(model);
                        caps.play.configure(
                            &model.config,
                            nodes.as_slice(),
                            &model.tuning.as_slice(),
                            InstrumentEV::PlayOpConfigure,
                        );
                    }
                }

                caps.render.render();
            }
            InstrumentEV::PlayOpPermission(grant) => {
                if grant {
                    caps.play.install_au(InstrumentEV::PlayOpInstall)
                } else {
                    caps.navigate.to(crate::Activity::Intro)
                }
            }
            InstrumentEV::RequestSnoops => caps.play.query_snoops(),
            InstrumentEV::PlayOpInstall(success) => {
                if !success {
                    self.update(InstrumentEV::Playback(PlaybackEV::Error), model, caps)
                } else {
                    model.setup_complete = true;
                    let nodes = self.get_nodes(model);
                    caps.play.configure(
                        &model.config,
                        nodes.as_slice(),
                        &model.tuning.as_slice(),
                        InstrumentEV::PlayOpConfigure,
                    );
                }
            }
            InstrumentEV::PlayOpConfigure(success) => {
                model.configured = success;
                if !success {
                    self.update(InstrumentEV::Playback(PlaybackEV::Error), model, caps)
                } else {
                    self.update(
                        InstrumentEV::Playback(PlaybackEV::Play(model.playing)),
                        model,
                        caps,
                    )
                }
            }
            InstrumentEV::PlayOpPause(success) => {
                if !success {
                    self.update(InstrumentEV::Playback(PlaybackEV::Error), model, caps)
                }
            }
            InstrumentEV::PlayOpPlay(success) => {
                if !success {
                    self.update(InstrumentEV::Playback(PlaybackEV::Error), model, caps)
                } else if !model.configured && model.playing {
                    let nodes = self.get_nodes(model);
                    caps.play.configure(
                        &model.config,
                        nodes.as_slice(),
                        &model.tuning.as_slice(),
                        InstrumentEV::PlayOpConfigure,
                    );
                }
            }
            InstrumentEV::Playback(playback_ev) => match playback_ev {
                PlaybackEV::Play(playing) => {
                    model.playing = playing;
                    model.snooped = vec![];
                    if !model.setup_complete {
                        caps.play.permissions(InstrumentEV::PlayOpPermission)
                    } else if playing {
                        caps.play.play(InstrumentEV::PlayOpPlay)
                    } else {
                        caps.play.pause(InstrumentEV::PlayOpPlay)
                    }
                    caps.render.render();
                }
                PlaybackEV::Error => {
                    model.playing = false;
                    model.setup_complete = false;
                    caps.render.render();
                }
            },
            InstrumentEV::SnoopData(d) => {
                model.snooped = d;

                let world = model.world.lock().expect("lock world");
                let mut outbound = model
                    .outbound
                    .as_ref()
                    .map(|e| world.get::<&mut OutboundString>(*e).ok())
                    .flatten()
                    .expect("get string");

                outbound.update_data(model.snooped.clone(), &model.config);
                caps.render.render();
            }
            InstrumentEV::NodeSnoopData(d) => {
                let mut world = model.world.lock().expect("lock world");
                for (f_n, d) in d {
                    let d_max: f32 = *d
                        .iter()
                        .max_by(|v, v1| {
                            if v.abs() > v1.abs() {
                                Ordering::Greater
                            } else if v.abs() < v1.abs() {
                                Ordering::Less
                            } else {
                                Ordering::Equal
                            }
                        })
                        .unwrap_or(&0.0);
                    let (_, node) = world
                        .query_mut::<&mut Node>()
                        .into_iter()
                        .find(|(_, node)| node.f_n == f_n)
                        .expect("node for f_n");
                    node.triggered = d_max.sin();
                }
                caps.render.render();
            }
            InstrumentEV::None => {}
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        InstrumentVM {
            nodes: self.get_nodes(model),
            playing: model.playing,
            config: model.config.clone(),
            layout: model.layout.clone().unwrap_or_default(),
            data_out: self.get_data_out(model),
        }
    }
}

impl Instrument {
    fn get_nodes(&self, model: &Model) -> Vec<Node> {
        let world = model.world.lock().expect("world lock");
        model
            .nodes
            .iter()
            .map(|e| *world.get::<&Node>(*e).expect("node for entity"))
            .collect()
    }

    fn get_data_out(&self, model: &Model) -> Vec<Point2<f64>> {
        let world = model.world.lock().expect("world lock");
        model
            .outbound
            .as_ref()
            .map(|e| {
                world
                    .get::<&OutboundString>(*e)
                    .expect("string for entity")
                    .data
                    .clone()
            })
            .unwrap_or_default()
    }
}
