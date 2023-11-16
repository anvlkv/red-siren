use crux_core::render::Render;
use crux_core::App;
use crux_kv::KeyValue;
use crux_macros::Effect;
use hecs::{Entity, World};
use serde::{Deserialize, Serialize};

pub mod config;
pub mod keyboard;
pub mod layout;
pub mod string;

pub use config::Config;
pub use layout::{Layout, LayoutRoot};

use crate::geometry::Rect;

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
}

#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct InstrumentVM {
    pub config: Config,
    pub layout: Layout,
    pub view_box: Rect,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum InstrumentEV {
    None,
    CreateWithConfig(Config),
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

                let layout = Layout::new(&model.world, &root).expect("Layout failed");

                let _ = model.root.insert(root);
                let _ = model.inbound.insert(inbound);
                let _ = model.outbound.insert(outbound);
                let _ = model.keyboard.insert(keyboard);
                let _ = model.layout.insert(layout);

                caps.render.render();
            }
            InstrumentEV::None => {}
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        InstrumentVM {
            config: model.config.clone(),
            layout: model.layout.clone().unwrap_or_default(),
            view_box: Rect::size(model.config.width, model.config.height)
        }
    }
}
