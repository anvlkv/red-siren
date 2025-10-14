use std::{cell::RefCell, collections::HashMap, mem};

use common::{
    instrument::{GroupChannel, GroupConfig},
    NodeKey,
};
use fundsp::hacker32::prelude::*;
use u_num_it::u_num_it;

use super::{
    node::{NodeType, S},
    InnerHandles, NodeHandles,
};

pub fn create_channel_system<G, K>(
    groups: &[GroupConfig],
    channel: usize,
    net: &mut Net,
) -> Vec<NodeHandles>
where
    G: Size<f32> + Size<NodeType>,
    K: Size<f32> + Size<NodeType>,
{
    let mut node_handles = Vec::<NodeHandles>::new();
    let mut inner_handles = Vec::<HashMap<NodeKey, InnerHandles>>::new();

    for group in groups.iter() {
        let mut group_handles = HashMap::<NodeKey, InnerHandles>::new();
        for node in group.nodes.iter() {
            let key = node.key;

            let (activation_snoop_front, activation_snoop_backend) =
                snoop(super::node::ACTIVATION_SNOOP_CAPACITY);
            let (output_snoop_front, output_snoop_backend) =
                snoop(super::node::OUTPUT_SNOOP_CAPACITY);
            let siren_control = shared(group.a_coef);
            let band_control = shared(0.0);

            group_handles.insert(
                key,
                InnerHandles {
                    key,
                    activation_snoop: activation_snoop_backend,
                    output_snoop: output_snoop_backend,
                    siren_control: Var::new(&siren_control),
                    band_control: Var::new(&band_control),
                },
            );

            node_handles.push(NodeHandles {
                key,
                activation_snoop: activation_snoop_front,
                output_snoop: output_snoop_front,
                siren_control,
                band_control,
            });
        }
        inner_handles.push(group_handles);
    }

    let groups = groups.to_vec();
    let handles_cell = RefCell::new(inner_handles);
    let node = busi::<G, _, _>(move |i| {
        let group_handles = mem::take(&mut handles_cell.borrow_mut()[i as usize]);
        super::node::create_group_node::<K>(&groups[i as usize], group_handles)
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

                            node_handles.extend(create_channel_system::<GNum, KNum>(
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
