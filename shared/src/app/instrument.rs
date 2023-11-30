pub mod config;
pub mod keyboard;
pub mod layout;
pub mod node;
pub mod string;

mod system;
use system::System;

use crate::geometry::Rect;
use crux_core::render::Render;
use crux_core::App;
use crux_macros::Effect;
use fundsp::audiounit::AudioUnit32;
use hecs::{Entity, World};
pub use node::Node;
use serde::{Deserialize, Serialize};

pub use config::Config;
pub use layout::{Layout, LayoutRoot};

pub const INPUT_STREAM_KV: &str = "input stream KV";
pub const OUTPUT_STREAM_KV: &str = "output stream KV";

#[derive(Default)]
pub struct Instrument;

#[derive(Default)]
pub struct Model {
    pub config: Config,
    pub world: World,
    pub inbound: Option<Entity>,
    pub outbound: Option<Entity>,
    pub keyboard: Option<Entity>,
    pub root: Option<Entity>,
    pub layout: Option<Layout>,
    pub playing: bool,
    pub system: Option<System>,
    // pub prev_audio_data: Vec<Vec<f32>>,
    pub audio_data: Vec<Vec<f32>>,
    pub frame_size: usize,
}

#[derive(Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct InstrumentVM {
    pub config: Config,
    pub view_box: Rect,
    pub nodes: Vec<Node>,
    pub playing: bool,
    pub audio_data: Vec<Vec<f32>>,
    pub layout: Layout,
}

impl Eq for InstrumentVM {}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum PlaybackEV {
    Play(bool),
    Error,
    DataIn(Vec<Vec<f32>>),
}

impl Eq for PlaybackEV {}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum InstrumentEV {
    None,
    CreateWithConfig(Config),
    Playback(PlaybackEV),
}

#[cfg_attr(feature = "typegen", derive(crux_macros::Export))]
#[derive(Effect)]
#[effect(app = "Instrument")]
pub struct InstrumentCapabilities {
    pub render: Render<InstrumentEV>,
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
                model.world = World::new();

                let inbound = string::InboundString::spawn(&mut model.world, &config);
                let outbound = string::OutboundString::spawn(&mut model.world, &config);
                let keyboard = keyboard::Keyboard::spawn(&mut model.world, &config);

                let root = layout::LayoutRoot::spawn(&mut model.world, inbound, outbound, keyboard);

                let layout = Layout::new(&model.world, &root, &config).expect("Layout failed");

                let _ = model.root.insert(root);
                let _ = model.inbound.insert(inbound);
                let _ = model.outbound.insert(outbound);
                let _ = model.keyboard.insert(keyboard);
                let _ = model.layout.insert(layout);
                let _ = model
                    .system
                    .insert(System::spawn(&mut model.world, &config));

                caps.render.render();
            }
            InstrumentEV::Playback(playback_ev) => match playback_ev {
                PlaybackEV::Play(playing) => {
                    if playing {
                        let sys = model.system.as_mut().unwrap();

                        model.playing = true;
                        sys.net_be.reset();
                        model.audio_data = vec![];
                        // model.prev_audio_data = vec![];
                    } else {
                        model.playing = false;
                    }
                    caps.render.render();
                }
                PlaybackEV::Error => {
                    model.playing = false;
                    model.audio_data = vec![];
                    // model.prev_audio_data = vec![];
                    caps.render.render();
                }
                PlaybackEV::DataIn(input) => {
                    if let Some(sys) = model.system.as_mut() {
                        let frame_size = input.first().map(|ch| ch.len()).unwrap_or_default();

                        if model.frame_size != frame_size {
                            model.audio_data = (0..model.config.channels)
                                .map(|_| (0..frame_size).map(|_| 0.0 as f32).collect())
                                .collect();
                            // model.prev_audio_data = model.audio_data.clone();
                        }

                        let input = input.iter().take(1).map(|ch| ch.as_slice()).collect::<Vec<_>>();
                        // let mut input_data = model.prev_audio_data.to_owned();
                        // input_data.extend(input.into_iter().nth(0));

                        // let input_data = input_data
                        //     .iter()
                        //     .map(|ch| ch.as_slice())
                        //     .collect::<Vec<_>>();

                        let mut output = model
                            .audio_data
                            .iter_mut()
                            .map(|ch| ch.as_mut_slice())
                            .collect::<Vec<_>>();

                        sys.net_be.process(
                            frame_size,
                            input.as_slice(),
                            output.as_mut_slice(),
                        );

                        caps.render.render();

                        // model.prev_audio_data = model.audio_data.clone();
                    } else {
                        log::warn!("skipping new data, no config yet");
                    }
                }
            },
            InstrumentEV::None => {}
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        let nodes = model
            .system
            .iter()
            .flat_map(|s| s.get_nodes(&model.world))
            .collect::<Vec<Node>>();

        InstrumentVM {
            nodes,
            playing: model.playing,
            config: model.config.clone(),
            layout: model.layout.clone().unwrap_or_default(),
            view_box: Rect::size(model.config.width, model.config.height),
            audio_data: model.audio_data.clone(),
        }
    }
}
