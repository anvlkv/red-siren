pub mod config;
pub mod keyboard;
pub mod layout;
pub mod node;
pub mod play;
pub mod string;

mod system;
use std::sync::{Arc, Mutex};

use system::System;

use crate::{geometry::Rect, util::Select};
use crux_core::render::Render;
use crux_core::App;
use crux_macros::Effect;
use fundsp::audiounit::AudioUnit32;
use hecs::{Entity, World};
pub use node::Node;
use serde::{Deserialize, Serialize};

pub use config::Config;
pub use layout::{Layout, LayoutRoot};

use self::play::PlayOperationOutput;

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
    pub system: Arc<Mutex<System>>,
    pub input_devices: Select<String>,
    pub output_devices: Select<String>,
}

#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct InstrumentVM {
    pub config: Config,
    pub view_box: Rect,
    pub nodes: Vec<Node>,
    pub playing: bool,
    pub layout: Layout,
    pub input_devices: Select<String>,
    pub output_devices: Select<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Eq)]
pub enum PlaybackEV {
    Play(bool),
    Error,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum InstrumentEV {
    None,
    CreateWithConfig(Config),
    UpdateConfig(Config),
    PlayOp(PlayOperationOutput),
    SelectInputDevice(String),
    SelectOutputDevice(String)
}

#[cfg_attr(feature = "typegen", derive(crux_macros::Export))]
#[derive(Effect)]
#[effect(app = "Instrument")]
pub struct InstrumentCapabilities {
    pub render: Render<InstrumentEV>,
    pub play: play::Play<InstrumentEV>,
}

impl App for Instrument {
    type Event = InstrumentEV;

    type Model = Model;

    type ViewModel = InstrumentVM;

    type Capabilities = InstrumentCapabilities;

    fn update(&self, event: Self::Event, model: &mut Self::Model, caps: &Self::Capabilities) {
        match event {
            InstrumentEV::CreateWithConfig(config) => {
                caps.play
                    .query_devices(InstrumentEV::PlayOp(PlayOperationOutput::Devices));
                self.update(InstrumentEV::UpdateConfig(config), model, caps);
            }
            InstrumentEV::UpdateConfig(config) => {
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
                model.system = match model.system.get_mut() {
                    Ok(sys) => *sys = System::spawn(&mut model.world, &config),
                    Err(e) => {
                        log::error!("system mtx poisoned");
                        let sys = *e.get_mut();
                        *sys = System::spawn(&mut model.world, &config)
                    }
                };

                caps.render.render();
            }
            InstrumentEV::PlayOp(playback_ev) => match playback_ev {
                PlayOperationOutput::Devices(inputs, outputs) => {
                    model.input_devices = Select::from(inputs).with_first_default();
                    model.output_devices = Select::from(outputs).with_first_default();
                    caps.render.render();
                }
            },
            InstrumentEV::SelectInputDevice(d) => {
                model.input_devices.select(&d);
                caps.render.render();
                if let Some(d) = model.input_devices.value() {
                    caps.play.configure_input(d.as_str());
                }
                else {
                    log::warn!("no valid input selected")
                }
            }
            InstrumentEV::SelectOutputDevice(d) => {
                model.output_devices.select(&d);
                caps.render.render();
                if let Some(d) = model.output_devices.value() {
                    caps.play.configure_output(d.as_str());
                }
                else {
                    log::warn!("no valid output selected")
                }
            }
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
            input_devices: model.input_devices.clone(),
            output_devices: model.output_devices.clone()
        }
    }
}
