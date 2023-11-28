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
use crux_kv::{KeyValue, KeyValueOutput};
use crux_macros::Effect;
use fundsp::{audiounit::AudioUnit32, buffer::Buffer};
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
    pub audio_data: Vec<Vec<f32>>,
}

#[derive(Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct InstrumentVM {
    pub config: Config,
    pub layout: Layout,
    pub view_box: Rect,
    pub nodes: Vec<Node>,
    pub playing: bool,
    pub audio_data: Vec<Vec<f32>>,
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
                    model.playing = playing;
                    caps.render.render();
                    if playing {
                        let sys = model.system.as_mut().unwrap();
                        sys.net_be.reset();
                    }
                }
                PlaybackEV::Error => todo!(),
                PlaybackEV::DataIn(input) => {
                    if let Some(sys) = model.system.as_mut() {
                        log::debug!("handling new data");
                        model.audio_data = vec![];
                        let sample_length = input.first().map(|ch| ch.len()).unwrap_or_default();
                        let rng = (0..sample_length).collect::<Vec<usize>>();
    
                        let mut it = rng.chunks(fundsp::MAX_BUFFER_SIZE).map(|range: &[usize]| {
                            let mut input_buffer = Buffer::<f32>::with_channels(input.len());
                            input.iter().zip(input_buffer.self_mut()).for_each(
                                |(input_ch, buffer_ch)| {
                                    input_ch
                                        .iter()
                                        .skip(range.first().cloned().unwrap_or(0))
                                        .take(fundsp::MAX_BUFFER_SIZE)
                                        .enumerate()
                                        .for_each(|(i, val)| {
                                            buffer_ch[i] = *val;
                                        });
                                },
                            );
    
                            let mut output = Buffer::<f32>::with_channels(model.config.channels);
    
                            let size = sample_length.min(fundsp::MAX_BUFFER_SIZE);
    
                            sys.net_be
                                .process(size, input_buffer.self_ref(), output.self_mut());
                            output
                        });
    
                        while let Some(mut output) = it.next() {
                            log::debug!("processed chunk: {:?}", output.self_ref());
                            model.audio_data.extend(
                                output
                                    .self_ref()
                                    .iter()
                                    .map(|s| Vec::from_iter(s.into_iter().cloned())),
                            )
                        }
    
                        caps.render.render();
                    }
                    else {
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
