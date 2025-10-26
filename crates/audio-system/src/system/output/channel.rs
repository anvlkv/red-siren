use std::{cell::RefCell, collections::HashMap, mem};

use common::{
    instrument::{GroupChannel, GroupConfig},
    NodeKey,
};
use fundsp::{hacker::prelude::*, typenum::op};
use u_num_it::u_num_it;

use super::{
    filter::{FilterHandles, FilterType},
    node::NodeType,
    InnerHandles, NodeHandles,
};

use crate::util::S;

pub fn create_channel_system<G, K, F>(
    groups: &[GroupConfig],
    channel: usize,
    net: &mut Net,
) -> Vec<NodeHandles>
where
    G: Size<S> + Size<NodeType>,
    K: Size<S> + Size<NodeType>,
    F: Size<S> + Size<FilterType>,
{
    let total_nodes: usize = groups.iter().map(|g| g.nodes.len()).sum();
    log::info!(
        "Creating channel system with {} groups, {} total nodes",
        groups.len(),
        total_nodes
    );

    let mut node_handles = Vec::<NodeHandles>::new();
    let mut inner_handles = Vec::<HashMap<NodeKey, InnerHandles>>::new();
    let mut filter_handles = Vec::<FilterHandles>::new();

    for (group_idx, group) in groups.iter().enumerate() {
        log::debug!(
            "Creating group {} with {} nodes",
            group_idx,
            group.nodes.len()
        );
        let mut group_handles = HashMap::<NodeKey, InnerHandles>::new();

        for node in group.nodes.iter() {
            let key = node.key;
            log::trace!("Creating node with key: {:?}", key);

            let (activation_snoop_front, activation_snoop_backend) =
                snoop(super::node::ACTIVATION_SNOOP_CAPACITY);
            let (output_snoop_front, output_snoop_backend) =
                snoop(super::node::OUTPUT_SNOOP_CAPACITY);
            let siren_control = shared(0.0);
            let band_control = shared(0.0);
            let key_control = shared(0.0);

            group_handles.insert(
                key,
                InnerHandles {
                    activation_snoop: activation_snoop_backend,
                    output_snoop: output_snoop_backend,
                    siren_control: Var::new(&siren_control),
                    band_control: Var::new(&band_control),
                },
            );

            filter_handles.push(FilterHandles {
                control: Var::new(&key_control),
                freq: node.base_frequency,
            });

            node_handles.push(NodeHandles {
                key,
                activation_snoop: activation_snoop_front,
                output_snoop: output_snoop_front,
                siren_control,
                band_control,
                key_control,
            });
            log::trace!("Created node handle for key: {:?}", key);
        }
        log::debug!(
            "Completed group {} with {} nodes",
            group_idx,
            group.nodes.len()
        );
        inner_handles.push(group_handles);
    }

    let groups = groups.to_vec();
    let handles_cell = RefCell::new(inner_handles);
    let node = busi::<G, _, _>(move |i| {
        let group_handles = mem::take(&mut handles_cell.borrow_mut()[i as usize]);
        super::node::create_group_node::<K>(&groups[i as usize], group_handles)
    }) >> pipei::<F, _, _>(move |i| {
        super::filter::create_filter(filter_handles[i as usize].clone())
    });
    let node_id = net.push(Box::new(node >> dcblock::<S>() >> declick::<S>()));
    net.connect_output(node_id, 0, channel);
    node_handles
}

pub fn one_channel_subsystem(
    channel_groups: &[GroupConfig],
    nodes_count_per_group: usize,
    channel: GroupChannel,
    net: &mut Net,
) -> Vec<NodeHandles> {
    let channel_groups_count = channel_groups.len();
    let mut node_handles = Vec::new();

    u_num_it!(
        1..=36,
        match channel_groups_count {
            U => {
                type GNum = NumType;

                u_num_it!(
                    [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71],
                    match nodes_count_per_group {
                        U => {
                            type KNum = NumType;

                            type FNum = op!(GNum * KNum);

                            node_handles.extend(create_channel_system::<GNum, KNum, FNum>(
                                channel_groups,
                                channel as usize,
                                net,
                            ));
                            log::info!("created stereo [{channel:?}] channel system: groups={channel_groups_count}, nodes={nodes_count_per_group}");
                        }
                        _ => {
                            panic!("unexpected number of nodes");
                        }
                    }
                );
            }
            _ => {
                panic!("unexpected number of groups in left channel");
            }
        }
    );

    node_handles
}
