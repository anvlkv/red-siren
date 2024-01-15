use app_core::{
    instrument::{Config, Node},
    play::PlayOperation,
    tuner::{TuningValue, MAX_F, MIN_F},
};
use crux_core::render::Render;
pub use crux_core::App;
use crux_macros::Effect;
use fundsp::hacker32::*;
use serde::{Deserialize, Serialize};
use spectrum_analyzer::{
    samples_fft_to_spectrum, scaling::divide_by_N_sqrt, windows::hann_window, FrequencyLimit,
};

use crate::{capture::Capture, system::SAMPLE_RATE};

use super::resolve::Resolve;
use super::system::System;

const ANALYZE_SAMPLES_COUNT: usize = 4096;

#[derive(Default)]
pub struct Model {
    system: Option<System>,
    config: Config,
    nodes: Vec<Node>,
    tuning: Vec<TuningValue>,
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
    pub capture: Capture<PlayOperation>,
}

impl App for RedSirenAU {
    type Event = PlayOperation;
    type Model = Model;
    type ViewModel = ViewModel;
    type Capabilities = RedSirenAUCapabilities;

    fn update(&self, msg: PlayOperation, model: &mut Model, caps: &RedSirenAUCapabilities) {
        log::trace!("au msg {msg:?}");
        match msg {
            PlayOperation::Config(config, nodes, tuning) => {
                model.config = config;
                model.nodes = nodes;
                model.tuning = tuning;
                _ = model.system.insert(System::new(
                    model.nodes.as_slice(),
                    &model.config,
                    model.tuning.as_slice(),
                ));

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
                            FrequencyLimit::Range(MIN_F, MAX_F),
                            Some(&divide_by_N_sqrt),
                        )
                        .unwrap();

                        caps.capture.capture_fft(Vec::from_iter(
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


                    let mut data = Vec::new();
                    while let Some (snp) = sys.snp.get() {
                        for i in 0..snp.size() {
                            data.push(snp.at(i))
                        }
                    }
                    if data.len() > 0 {
                        caps.capture.capture_data(data);
                    }
                } else {
                    log::warn!("skipping new data, no system yet, nor capturing");
                }
            }
            PlayOperation::Capture(capturing) => {
                model.capturing = capturing;
                caps.resolve.resolve_success(true);
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
