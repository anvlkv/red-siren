pub mod excitement_control;
pub mod input;
pub mod output;
pub mod output_analyzer;
pub mod values;

use std::collections::HashMap;

use crate::rt::ExcitementSource;
use common::NodeKey;
use fundsp::{
    net::Net,
    prelude::{var, An},
    shared::Shared,
    snoop::{Snoop, SnoopBackend},
};
use values::FineTunedValues;

pub use excitement_control::ExcitementControl;

pub struct NodeHandles {
    pub key: NodeKey,
    pub excitement_snoop: Snoop,
    pub secondary_excitement_snoop: Snoop,
    pub output_snoop: Snoop,
    pub siren_control: ExcitementControl,
    pub band_control: Shared,
    pub key_control: Shared,
}

pub struct SensorHandles {
    pub key: NodeKey,
    pub min_frequency: Shared,
    pub max_frequency: Shared,
    pub min_magnitude: Shared,
    pub max_magnitude: Shared,
}

/// Create output subsystem for live playback.
#[must_use]
pub fn create_output_system(
    config: &common::instrument::Config,
    net: &mut Net,
    num_channels: usize,
    #[cfg(feature = "editor")] values: &FineTunedValues,
) -> Vec<NodeHandles> {
    #[cfg(not(feature = "editor"))]
    let values = &FineTunedValues::new();

    log::debug!("Creating output system with fine-tuned values: {values:#?}");

    match num_channels {
        1 => output::mono_system(config, net, values),
        2 => output::stereo_system(config, net, values),
        3.. => output::multi_channel_system(config, net, num_channels, values),
        0 => {
            panic!("Number of output channels cannot be zero");
        }
    }
}

#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn create_input_system(
    config: &common::tuner::Config,
    net: &mut Net,
    excitements: HashMap<NodeKey, ExcitementControl>,
    source: ExcitementSource,
    spectrum_thb: &input::analyzer::SpectrumBuffer,
    input_snoop: Option<An<SnoopBackend>>,
    (min_freq, max_freq): (&Shared, &Shared),
    (ny_threshold, wet_ratio): (&Shared, &Shared),
    tap_channel: usize,
) -> Vec<SensorHandles> {
    match source {
        ExcitementSource::Mic => {
            // Use FFT analyzer for microphone input
            input::sensors_system(
                config,
                net,
                excitements,
                var(ny_threshold),
                var(wet_ratio),
                var(min_freq),
                var(max_freq),
                spectrum_thb,
                tap_channel,
                input_snoop,
            )
        }
        ExcitementSource::Entropy => {
            // Use random excitor for entropy source
            input::randomized_system(config, net, excitements);

            vec![]
        }
    }
}

#[cfg(test)]
mod tests;
