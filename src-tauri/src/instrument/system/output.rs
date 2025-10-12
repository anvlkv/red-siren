mod channel;
mod node;
mod siren;

use channel::one_channel_subsystem;
use common::instrument::{Config, GroupChannel};
use common::NodeKey;
use fundsp::hacker32::prelude::*;

use super::NodeHandles;

#[allow(dead_code)]
struct InnerHandles {
    key: NodeKey,
    activation_snoop: An<SnoopBackend>,
    output_snoop: An<SnoopBackend>,
    siren_control: Var,
    band_control: Var,
}

impl Default for InnerHandles {
    fn default() -> Self {
        let (_, activation_snoop) = snoop(node::ACTIVATION_SNOOP_CAPACITY);
        let (_, output_snoop) = snoop(node::OUTPUT_SNOOP_CAPACITY);
        let siren_control = shared(0.0);
        let band_control = shared(0.0);

        Self {
            key: NodeKey(0, 0),
            activation_snoop,
            output_snoop,
            siren_control: Var::new(&siren_control),
            band_control: Var::new(&band_control),
        }
    }
}

pub fn mono_system(config: &Config, net: &mut Net) -> Vec<NodeHandles> {
    let nodes_count_per_group = config.num_nodes_per_group();
    let groups = config.0.as_slice();

    one_channel_subsystem(groups, nodes_count_per_group, GroupChannel::Left, net)
}

pub fn stereo_system(config: &Config, net: &mut Net) -> Vec<NodeHandles> {
    let nodes_count_per_group = config.num_nodes_per_group();
    let groups_count_left = config.num_groups_left();
    let groups_count_right = config.num_groups_right();

    let left_groups = (0..groups_count_left)
        .filter_map(|gi| config.group_nth_channel(GroupChannel::Left, gi))
        .cloned()
        .collect::<Vec<_>>();
    let right_groups = (0..groups_count_right)
        .filter_map(|gi| config.group_nth_channel(GroupChannel::Right, gi))
        .cloned()
        .collect::<Vec<_>>();

    let mut node_handles = Vec::new();

    node_handles.extend(one_channel_subsystem(
        left_groups.as_slice(),
        nodes_count_per_group,
        GroupChannel::Left,
        net,
    ));
    node_handles.extend(one_channel_subsystem(
        right_groups.as_slice(),
        nodes_count_per_group,
        GroupChannel::Right,
        net,
    ));

    node_handles
}

pub fn multi_channel_system(
    _config: &Config,
    _net: &mut Net,
    _num_channels: usize,
) -> Vec<NodeHandles> {
    todo!("multi_channel_system")
}
