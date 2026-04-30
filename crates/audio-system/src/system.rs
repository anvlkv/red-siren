use common::instrument::Config as InstrumentConfig;
use fundsp::prelude::*;

pub mod excitor;
pub mod feedback_pass;
pub mod grid;
pub mod node;
pub mod values;

#[cfg(feature = "editor")]
use values::FineTunedValues;

use crate::{SampleType, rt::ExcitementSource};

/// Mounts the audio output system
pub fn mount_output_system(
    main_net: &mut Net,
    num_channels: usize,
    sample_type: SampleType,
    config: &InstrumentConfig,
    input_net_id: Option<NodeId>,
) -> NodeId {
    let mut subnet = Net::new(config.num_nodes_total(), num_channels);

    let subnet_id = main_net.push(Box::new(subnet));

    main_net.pipe_output(subnet_id);

    if let Some(input_id) = input_net_id {
        main_net.pipe_all(input_id, subnet_id);
    }

    subnet_id
}

pub fn mount_input_system(
    main_net: &mut Net,
    config: &InstrumentConfig,
    excitment_src: ExcitementSource,
) {
}
