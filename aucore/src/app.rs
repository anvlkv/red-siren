use app_core::{
    instrument::{Config, Node},
    play::PlayOperation,
};
use crux_core::render::Render;
pub use crux_core::App;
use crux_macros::Effect;
use fundsp::hacker32::*;
use serde::{Deserialize, Serialize};
use spectrum_analyzer::windows::hann_window;
use spectrum_analyzer::{samples_fft_to_spectrum, FrequencyLimit};

use crate::system::SAMPLE_RATE;

use super::resolve::Resolve;
use super::system::System;

const ANALYZE_SAMPLES_COUNT: usize = 2048;

#[derive(Default)]
pub struct Model {
    system: Option<System>,
    config: Config,
    nodes: Vec<Node>,
    audio_data: Vec<Vec<f32>>,
    analyze_samples: Vec<f32>,
    frame_size: usize,
    capturing: bool,
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
        log::trace!("au msg: {msg:?}");
        
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
                if model.capturing {
                    let data = input.first().cloned().unwrap_or(vec![]);
                    if model.analyze_samples.len() < ANALYZE_SAMPLES_COUNT {
                        model.analyze_samples.extend(data)
                    } else {
                        let samples = std::mem::replace(&mut model.analyze_samples, data);

                        let hann_window = hann_window(samples.as_slice());

                        let spectrum_hann_window = samples_fft_to_spectrum(
                            &hann_window,
                            SAMPLE_RATE as u32,
                            FrequencyLimit::All,
                            None,
                        )
                        .unwrap();

                        caps.resolve.resolve_capture_fft(Vec::from_iter(
                            spectrum_hann_window
                                .data()
                                .iter()
                                .map(|(freq, value)| (freq.val(), value.val())),
                        ));
                    }
                } else if let Some(sys) = model.system.as_mut() {
                    let frame_size = input.first().map_or(0, |ch| ch.len());
                    let channels = sys.channels;
                    if frame_size != model.frame_size || model.audio_data.len() != channels {
                        if model.frame_size > 0 {
                            log::warn!("resizing at runtime")
                        }

                        model.frame_size = frame_size;
                        model.audio_data = (0..channels)
                            .map(|_| (0..frame_size).map(|_| 0_f32).collect())
                            .collect();
                    }

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
                    log::warn!("skipping new data, no system yet, nor capturing");
                }
            }
            PlayOperation::Capture(capturing) => {
                model.capturing = capturing;
                if !capturing {
                    caps.resolve.resolve_success(true);
                } else {
                    // model.capture_id = id;
                    caps.render.render();
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
