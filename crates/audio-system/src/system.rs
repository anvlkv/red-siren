pub mod input;
mod output;
pub mod values;

use std::collections::HashMap;

use crate::rt::ActivationSource;
use common::NodeKey;
use fundsp::{net::Net, shared::Shared, snoop::Snoop};
use values::FineTunedValues;

pub struct NodeHandles {
    pub key: NodeKey,
    pub activation_snoop: Snoop,
    pub output_snoop: Snoop,
    pub siren_control: Shared,
    pub band_control: Shared,
    pub key_control: Shared,
}

/// Create output subsystem for live playback.
pub fn create_output_system(
    config: &common::instrument::Config,
    net: &mut Net,
    num_channels: usize,
    #[cfg(feature = "editor")] values: &FineTunedValues,
) -> Vec<NodeHandles> {
    #[cfg(not(feature = "editor"))]
    let values = &FineTunedValues::new();

    match num_channels {
        1 => output::mono_system(config, net, values),
        2 => output::stereo_system(config, net, values),
        3.. => output::multi_channel_system(config, net, num_channels, values),
        0 => {
            panic!("Number of output channels cannot be zero");
        }
    }
}

pub fn create_input_system(
    config: &common::tuner::Config,
    net: &mut Net,
    activations: HashMap<NodeKey, Shared>,
    source: ActivationSource,
    #[cfg(feature = "editor")] values: &FineTunedValues,
) {
    #[cfg(not(feature = "editor"))]
    let values = &FineTunedValues::new();

    match source {
        ActivationSource::Mic => {
            // Use FFT analyzer for microphone input
            input::sensors_system(config, net, activations, values)
        }
        ActivationSource::Entropy => {
            // Use random activator for entropy source
            input::randomized_system(config, net, activations)
        }
    }
}

#[cfg(test)]
mod tests;
