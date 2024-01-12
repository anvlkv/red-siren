use std::sync::{Arc, Mutex};

use crux_core::render::Render;
use crux_core::App;
use crux_macros::Effect;
use hecs::{Entity, World};
use serde::{Deserialize, Serialize};

pub use config::Config;
pub use layout::{Layout, LayoutRoot};
use node::spawn_all_nodes;
pub use node::Node;

use crate::{play::Play, Navigate};

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
}

impl Eq for InstrumentVM {}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum PlaybackEV {
    Play(bool),
    Error,
}

impl Eq for PlaybackEV {}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum InstrumentEV {
    None,
    CreateWithConfig(Config),
    Playback(PlaybackEV),
    PlayOpPermission(bool),
    PlayOpInstall(bool),
    PlayOpConfigure(bool),
    PlayOpPlay(bool),
    PlayOpPause(bool),
}

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

                if model.setup_complete {
                    let nodes = self.get_nodes(model);
                    caps.play.configure(
                        &model.config,
                        nodes.as_slice(),
                        InstrumentEV::PlayOpConfigure,
                    );
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
            InstrumentEV::PlayOpInstall(success) => {
                if !success {
                    self.update(InstrumentEV::Playback(PlaybackEV::Error), model, caps)
                } else {
                    model.setup_complete = true;
                    let nodes = self.get_nodes(model);
                    caps.play.configure(
                        &model.config,
                        nodes.as_slice(),
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
                }
            }
            InstrumentEV::Playback(playback_ev) => match playback_ev {
                PlaybackEV::Play(playing) => {
                    model.playing = playing;
                    if !model.setup_complete {
                        caps.play.permissions(InstrumentEV::PlayOpPermission)
                    } else if !model.configured {
                        let nodes = self.get_nodes(model);
                        caps.play.configure(
                            &model.config,
                            nodes.as_slice(),
                            InstrumentEV::PlayOpConfigure,
                        );
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
            InstrumentEV::None => {}
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        InstrumentVM {
            nodes: self.get_nodes(model),
            playing: model.playing,
            config: model.config.clone(),
            layout: model.layout.clone().unwrap_or_default(),
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
}
