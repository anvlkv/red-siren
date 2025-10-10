mod input;
mod output;
mod siren;

use std::collections::HashMap;

use fundsp::{net::Net, shared::Shared, snoop::Snoop};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeKey(pub u8, pub u8);

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
    config: &common::tuner::TunerData,
    net: &mut Net,
    sirens: HashMap<NodeKey, Shared>,
) {
}
