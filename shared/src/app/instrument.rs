pub mod config;
pub mod keyboard;
pub mod layout;
pub mod node;
pub mod string;
mod system;

use crate::geometry::Rect;
use crux_core::render::Render;
use crux_core::App;
use crux_kv::KeyValue;
use crux_macros::Effect;
use hecs::{Entity, World};
pub use node::Node;
use serde::{Deserialize, Serialize};
use system::System;

pub use config::Config;
pub use layout::{Layout, LayoutRoot};

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
    pub system: Option<System>,
    pub playing: bool,
}

#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct InstrumentVM {
    pub config: Config,
    pub layout: Layout,
    pub view_box: Rect,
    pub nodes: Vec<Node>,
    pub playing: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum InstrumentEV {
    None,
    CreateWithConfig(Config),
    Playback(bool),
}

#[cfg_attr(feature = "typegen", derive(crux_macros::Export))]
#[derive(Effect)]
#[effect(app = "Instrument")]
pub struct InstrumentCapabilities {
    pub render: Render<InstrumentEV>,
    pub key_value: KeyValue<InstrumentEV>,
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
            InstrumentEV::Playback(playing) => {
                model.playing = playing;
                caps.render.render();
                if playing {
                    caps.key_value.write(key, value, make_event)
                }
            }
            InstrumentEV::None => {}
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        let nodes = model.system
            .iter()
            .flat_map(|s| s.get_nodes(&model.world))
            .collect::<Vec<Node>>();

        InstrumentVM {
            nodes,
            playing: model.playing,
            config: model.config.clone(),
            layout: model.layout.clone().unwrap_or_default(),
            view_box: Rect::size(model.config.width, model.config.height),
        }
    }
}
