use ::shared::{
    instrument::{Config, Node},
    play::PlayOperation,
};
use crux_core::render::Render;
pub use crux_core::App;
use crux_macros::Effect;
use fundsp::hacker32::*;
use serde::{Deserialize, Serialize};

use super::resolve::Resolve;
use super::system::System;

#[derive(Default)]
pub struct Model {
    system: Option<System>,
    config: Config,
    nodes: Vec<Node>,
    audio_data: Vec<Vec<f32>>,
    frame_size: usize,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct ViewModel(pub Vec<Vec<f32>>);

#[derive(Default)]
pub struct RedSirenAU;

#[cfg_attr(feature = "typegen", derive(crux_macros::Export))]
#[derive(Effect)]
#[effect(app = "RedSirenAU")]
pub struct RedSirenAUCapabilities {
    pub render: Render<PlayOperation>,
    pub resolve: Resolve<PlayOperation>,
}

impl App for RedSirenAU {
    type Event = PlayOperation;
    type Model = Model;
    type ViewModel = ViewModel;
    type Capabilities = RedSirenAUCapabilities;

    fn update(&self, msg: PlayOperation, model: &mut Model, caps: &RedSirenAUCapabilities) {
        match msg {
            PlayOperation::Config(config, nodes) => {
                model.config = config;
                model.nodes = nodes;
                _ = model
                    .system
                    .insert(System::new(model.nodes.as_slice(), &model.config));

                caps.render.render();
                caps.resolve.resolve_success(true);
            }
            PlayOperation::Input(input) => {
                let frame_size = input.first().map_or(0, |ch| ch.len());
                let channels = model.system.as_ref().unwrap().channels;
                if frame_size != model.frame_size || model.audio_data.len() != channels {
                    if model.frame_size > 0 {
                        log::warn!("resizing at runtime")
                    }

                    model.frame_size = frame_size;
                    model.audio_data = (0..channels)
                        .map(|_| (0..frame_size).map(|_| 0_f32).collect())
                        .collect();
                }

                if let Some(sys) = model.system.as_mut() {
                    let input = input
                        .iter()
                        .take(1)
                        .map(|ch| ch.as_slice())
                        .collect::<Vec<_>>();

                    let mut output = model
                        .audio_data
                        .iter_mut()
                        .map(|ch| ch.as_mut_slice())
                        .collect::<Vec<_>>();

                    sys.net_be
                        .process(model.frame_size, input.as_slice(), output.as_mut_slice());

                    caps.render.render();
                } else {
                    log::warn!("skipping new data, no system yet");
                }
            }
            op => {
                log::debug!("op: {op:?} reached hard bottom");
                caps.resolve.resolve_success(true);
            }
        }
    }

    fn view(&self, model: &Model) -> ViewModel {
        ViewModel(model.audio_data.clone())
    }
}

#[cfg(test)]
mod tests {}
