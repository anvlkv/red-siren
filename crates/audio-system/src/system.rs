pub mod input;
mod output;

use std::collections::HashMap;

use common::NodeKey;
use fundsp::{net::Net, shared::Shared, snoop::Snoop};

pub struct NodeHandles {
    pub key: NodeKey,
    pub activation_snoop: Snoop,
    pub output_snoop: Snoop,
    pub siren_control: Shared,
    pub band_control: Shared,
}

pub fn create_output_system(
    config: &common::instrument::Config,
    net: &mut Net,
    num_channels: usize,
) -> Vec<NodeHandles> {
    match num_channels {
        1 => output::mono_system(config, net),
        2 => output::stereo_system(config, net),
        3.. => output::multi_channel_system(config, net, num_channels),
        0 => {
            panic!("Number of output channels cannot be zero");
        }
    }
}

pub fn create_input_system(
    config: &common::tuner::Config,
    net: &mut Net,
    activations: HashMap<NodeKey, Shared>,
) {
    input::sensors_system(config, net, activations)
}
