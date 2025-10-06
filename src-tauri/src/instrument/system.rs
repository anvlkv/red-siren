mod input;
mod output;

use fundsp::{net::Net, shared::Shared};

pub fn create_output_system(
    config: &common::instrument::Config,
    net: &mut Net,
    num_channels: usize,
) {
    match num_channels {
        1 => {
            output::mono_system(config, net);
        }
        2 => {
            output::stereo_system(config, net);
        }
        3.. => {
            output::multi_channel_system(config, net, num_channels);
        }
        0 => {
            panic!("Number of output channels cannot be zero");
        }
    }
}

pub fn create_input_system(config: &common::tuner::Data, net: &mut Net) -> Vec<Shared> {
    vec![]
}
