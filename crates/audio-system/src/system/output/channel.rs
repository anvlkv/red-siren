use std::{cell::RefCell, collections::HashMap, mem};

use common::{
    instrument::{GroupChannel, GroupConfig},
    NodeKey,
};
use fundsp::hacker::prelude::*;
use u_num_it::u_num_it;

use super::{
    filter::{FilterHandles, FilterType},
    node::NodeType,
    InnerHandles,
};

use crate::system::values::FineTunedValues;
use crate::util::S;

pub fn one_channel_subsystem(
    channel_groups: &[GroupConfig],
    (group_handles, filter_handles): (Vec<HashMap<NodeKey, InnerHandles>>, Vec<FilterHandles>),
    nodes_count_per_group: usize,
    channel: GroupChannel,
    net: &mut Net,
    values: &FineTunedValues,
) -> NodeId {
    let channel_groups_count = channel_groups.len();
    let channel_filters_count = filter_handles.len();
    let mut id: NodeId = u_num_it!(
        1..=36,
        match channel_groups_count {
            U => {
                type GNum = NumType;

                u_num_it!(
                    [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71],
                    match nodes_count_per_group {
                        U => {
                            type KNum = NumType;

                            let id = create_channel_system::<GNum, KNum>(
                                channel_groups,
                                group_handles,
                                net,
                                values,
                            );
                            log::info!("created stereo [{channel:?}] channel system: groups={channel_groups_count}, nodes={nodes_count_per_group}");

                            id
                        }
                        _ => {
                            panic!("unexpected number of nodes");
                        }
                    }
                )
            }
            _ => {
                panic!("unexpected number of groups in [{channel:?}]");
            }
        }
    );

    u_num_it!(
        1..=71,
        match channel_filters_count {
            U => {
                type FNum = NumType;
                id = create_channel_filter::<FNum>(filter_handles, id, net, values);
            }
            _ => {
                panic!("unexpected number of filters");
            }
        }
    );

    id
}

fn create_channel_system<G, K>(
    groups: &[GroupConfig],
    handles: Vec<HashMap<NodeKey, InnerHandles>>,
    net: &mut Net,
    values: &FineTunedValues,
) -> NodeId
where
    G: Size<S> + Size<NodeType>,
    K: Size<S> + Size<NodeType>,
{
    let total_nodes: usize = groups.iter().map(|g| g.nodes.len()).sum();
    log::info!(
        "Creating channel system with {} groups, {} total nodes",
        groups.len(),
        total_nodes
    );

    let groups = groups.to_vec();
    let group_handles_cell = RefCell::new(handles);

    let node = busi::<G, _, _>({
        let values = values.clone();
        move |i| {
            let group_handles = mem::take(&mut group_handles_cell.borrow_mut()[i as usize]);
            super::node::create_group_node::<K>(&groups[i as usize], group_handles, &values)
        }
    });
    net.push(Box::new(node))
}

fn create_channel_filter<F>(
    filter_handles: Vec<FilterHandles>,
    src_id: NodeId,
    net: &mut Net,
    values: &FineTunedValues,
) -> NodeId
where
    F: Size<S> + Size<FilterType>,
{
    log::info!(
        "Creating channel filter with {} filters",
        filter_handles.len(),
    );

    let node = pass()
        >> pipei::<F, _, _>({
            let values = values.clone();
            move |i| super::filter::create_filter(filter_handles[i as usize].clone(), &values)
        });
    let filter_id = net.push(Box::new(node >> dcblock::<S>() >> declick::<S>()));
    net.pipe_all(src_id, filter_id);

    filter_id
}
