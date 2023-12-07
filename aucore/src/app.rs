use super::resolve::Resolve;
use super::system::System;
use ::shared::{
    instrument::{Config, Node},
    play::PlayOperation,
};
pub use crux_core::App;
use crux_core::render::Render;
use crux_macros::Effect;
use fundsp::hacker32::*;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Model {
    system: Option<System>,
    config: Config,
    nodes: Vec<Node>,
    audio_data: Vec<Vec<f32>>,
    frame_size: usize,
}

#[derive(Serialize, Deserialize, Default, Clone)]
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
    type Model = Model;
    type Event = PlayOperation;
    type ViewModel = ViewModel;
    type Capabilities = RedSirenAUCapabilities;

    fn update(&self, msg: PlayOperation, model: &mut Model, caps: &RedSirenAUCapabilities) {
        log::trace!("msg: {:?}", msg);

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
            PlayOperation::Resume => {}
            PlayOperation::Suspend => {}
            PlayOperation::InstallAU => {}
            PlayOperation::Permissions => {}
            PlayOperation::Input(input) => {
                if let Some(sys) = model.system.as_mut() {
                    let frame_size = input.first().map(|ch| ch.len()).unwrap_or_default();

                    if model.frame_size != frame_size {
                        model.audio_data = (0..sys.channels)
                            .map(|_| (0..frame_size).map(|_| 0.0_f32).collect())
                            .collect();
                        // model.prev_audio_data = model.audio_data.clone();
                    }

                    let input = input
                        .iter()
                        .take(1)
                        .map(|ch| ch.as_slice())
                        .collect::<Vec<_>>();
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

                    sys.net_be
                        .process(frame_size, input.as_slice(), output.as_mut_slice());

                    caps.render.render();

                    // model.prev_audio_data = model.audio_data.clone();
                } else {
                    log::warn!("skipping new data, no system yet");
                }
            }
            PlayOperation::QueryInputDevices => todo!(),
            PlayOperation::QueryOutputDevices => todo!(),
        }
    }

    fn view(&self, model: &Model) -> ViewModel {
        ViewModel(model.audio_data.clone())
    }
}

#[cfg(test)]
mod tests {}
