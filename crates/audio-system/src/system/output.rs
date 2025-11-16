mod abs;
mod channel;
mod crossfade;
mod div;
mod filter;
mod formant;
mod lpc;
mod node;
mod pow;
mod siren;

use std::collections::HashMap;

use channel::one_channel_subsystem;
use common::{
    instrument::{Config, GroupChannel, GroupConfig},
    NodeKey,
};
#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;

use super::NodeHandles;
use crate::{
    output::{filter::FilterHandles, lpc::lpc},
    system::values::FineTunedValues,
    util::S,
};

#[derive(Clone)]
struct InnerHandles {
    excitement_snoop: An<SnoopBackend>,
    output_snoop: An<SnoopBackend>,
    siren_control: Var,
    band_control: Var,
}

impl Default for InnerHandles {
    fn default() -> Self {
        let (_, excitement_snoop) = snoop(node::ACTIVATION_SNOOP_CAPACITY);
        let (_, output_snoop) = snoop(node::OUTPUT_SNOOP_CAPACITY);
        let siren_control = shared(0.0);
        let band_control = shared(0.0);

        Self {
            excitement_snoop,
            output_snoop,
            siren_control: Var::new(&siren_control),
            band_control: Var::new(&band_control),
        }
    }
}

pub fn mono_system(config: &Config, net: &mut Net, values: &FineTunedValues) -> Vec<NodeHandles> {
    let nodes_count_per_group = config.num_nodes_per_group();
    let groups = config.0.as_slice();

    let (node_handles, group_handles, filter_handles) = prepare_handles(groups);

    let id = one_channel_subsystem(
        groups,
        (group_handles, filter_handles),
        nodes_count_per_group,
        GroupChannel::Left,
        net,
        values,
    );

    let system_filter = split::<U2>() >> ((pinkpass::<S>() >> lpc::<7>()) | pass()) >> join::<U2>();

    let system_filter_id = net.push(Box::new(system_filter));

    net.pipe_all(id, system_filter_id);

    net.pipe_output(system_filter_id);

    node_handles
}

pub fn stereo_system(config: &Config, net: &mut Net, values: &FineTunedValues) -> Vec<NodeHandles> {
    log::info!(
        "Creating stereo output system with {} total groups",
        config.num_groups()
    );

    let (node_handles, group_handles, filter_handles) = prepare_handles(&config.0);

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

    let (left_group_handles, right_group_handles) = group_handles.into_iter().fold(
        (
            Vec::<HashMap<NodeKey, InnerHandles>>::new(),
            Vec::<HashMap<NodeKey, InnerHandles>>::new(),
        ),
        |(mut left, mut right), handles| {
            if handles
                .keys()
                .any(|k| config.channel_of_key(k) == Some(GroupChannel::Left))
            {
                left.push(handles);
            } else if handles
                .keys()
                .any(|k| config.channel_of_key(k) == Some(GroupChannel::Right))
            {
                right.push(handles);
            } else {
                log::error!("Invalid group channel");
            }
            (left, right)
        },
    );

    let (left_filter_handles, right_filter_handles) = filter_handles.into_iter().fold(
        (Vec::<FilterHandles>::new(), Vec::<FilterHandles>::new()),
        |(mut left, mut right), handle| {
            match config.channel_of_key(&handle.config.key) {
                Some(GroupChannel::Left) => left.push(handle),
                Some(GroupChannel::Right) => right.push(handle),
                _ => log::error!("Invalid group channel"),
            }

            (left, right)
        },
    );

    let left_id = one_channel_subsystem(
        left_groups.as_slice(),
        (left_group_handles, right_filter_handles),
        nodes_count_per_group,
        GroupChannel::Left,
        net,
        values,
    );
    let right_id = one_channel_subsystem(
        right_groups.as_slice(),
        (right_group_handles, left_filter_handles),
        nodes_count_per_group,
        GroupChannel::Right,
        net,
        values,
    );

    let system_filter = split::<U2>()
        >> ((pinkpass::<S>() >> lpc::<3>()) | pass())
        >> (pan(0.15) | pan(0.85))
        >> (pass() | reverse::<U2>() | pass());

    let left_filter_id = net.push(Box::new(system_filter.clone()));
    let right_filter_id = net.push(Box::new(system_filter));

    net.pipe_all(left_id, left_filter_id);
    net.pipe_all(right_id, right_filter_id);

    let mapper = map(|frame: &Frame<f32, U4>| {
        let treated = frame[0];
        let untreated_mixin = frame[1];
        let treated_mixin = frame[2];
        let untreated = frame[3];

        (treated + untreated_mixin + treated_mixin + untreated) / 4.0
    });

    let system_join = multipass::<U8>()
        >> (pass() | reverse::<U5>() | multipass::<U2>())
        >> (multipass::<U4>() | reverse::<U3>() | pass())
        >> (mapper.clone() | mapper.clone());

    let join_id = net.push(Box::new(system_join));

    net.pipe_all(left_filter_id, join_id);
    net.pipe_all(right_filter_id, join_id);

    net.pipe_output(join_id);

    node_handles
}

pub fn multi_channel_system(
    config: &Config,
    net: &mut Net,
    num_channels: usize,
    values: &FineTunedValues,
) -> Vec<NodeHandles> {
    log::info!(
        "Creating stereo output system with {} total groups",
        config.num_groups()
    );

    let (node_handles, group_handles, filter_handles) = prepare_handles(&config.0);

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

    let (left_group_handles, right_group_handles) = group_handles.into_iter().fold(
        (
            Vec::<HashMap<NodeKey, InnerHandles>>::new(),
            Vec::<HashMap<NodeKey, InnerHandles>>::new(),
        ),
        |(mut left, mut right), handles| {
            if handles
                .keys()
                .any(|k| config.channel_of_key(k) == Some(GroupChannel::Left))
            {
                left.push(handles);
            } else if handles
                .keys()
                .any(|k| config.channel_of_key(k) == Some(GroupChannel::Right))
            {
                right.push(handles);
            } else {
                log::error!("Invalid group channel");
            }
            (left, right)
        },
    );

    let (left_filter_handles, right_filter_handles) = filter_handles.into_iter().fold(
        (Vec::<FilterHandles>::new(), Vec::<FilterHandles>::new()),
        |(mut left, mut right), handle| {
            match config.channel_of_key(&handle.config.key) {
                Some(GroupChannel::Left) => left.push(handle),
                Some(GroupChannel::Right) => right.push(handle),
                _ => log::error!("Invalid group channel"),
            }

            (left, right)
        },
    );

    let left_id = one_channel_subsystem(
        left_groups.as_slice(),
        (left_group_handles, right_filter_handles),
        nodes_count_per_group,
        GroupChannel::Left,
        net,
        values,
    );
    let right_id = one_channel_subsystem(
        right_groups.as_slice(),
        (right_group_handles, left_filter_handles),
        nodes_count_per_group,
        GroupChannel::Right,
        net,
        values,
    );

    // Creative multi-channel mapping:
    // - Process each side through a widening/filter block (same as stereo),
    // - Then fold 8 channels (4 per side) down to the requested number of outputs
    //   using contiguous grouping with slightly weighted averages for a bit of color.

    // Side processing: widen into 4 channels per side with light coloration.
    let system_filter = split::<U2>()
        >> ((pinkpass::<S>() >> lpc::<3>()) | pass())
        >> (pan(0.12) | pan(0.88))
        >> (pass() | reverse::<U2>() | pass());

    let left_filter_id = net.push(Box::new(system_filter.clone()));
    let right_filter_id = net.push(Box::new(system_filter));

    net.pipe_all(left_id, left_filter_id);
    net.pipe_all(right_id, right_filter_id);

    // Small helper mappers.
    let map1 = map(|f: &Frame<f32, U1>| f[0]);
    let map2 = map(|f: &Frame<f32, U2>| 0.6 * f[0] + 0.4 * f[1]);
    let map3 = map(|f: &Frame<f32, U3>| 0.5 * f[0] + 0.3 * f[1] + 0.2 * f[2]);

    let join_id = match num_channels {
        3 => {
            // Groups: [L0,L1,L2] | [L3,R0] | [R1,R2,R3]
            let system_join = multipass::<U8>() >> (map3.clone() | map2.clone() | map3.clone());
            net.push(Box::new(system_join))
        }
        4 => {
            // Groups: [L0,L1] | [L2,L3] | [R0,R1] | [R2,R3]
            let system_join =
                multipass::<U8>() >> (map2.clone() | map2.clone() | map2.clone() | map2.clone());
            net.push(Box::new(system_join))
        }
        5 => {
            // Groups: [L0,L1] | [L2] | [L3,R0] | [R1] | [R2,R3]
            let system_join = multipass::<U8>()
                >> (map2.clone() | map1.clone() | map2.clone() | map1.clone() | map2.clone());
            net.push(Box::new(system_join))
        }
        6 => {
            // Groups: [L0] | [L1,L2] | [L3] | [R0] | [R1,R2] | [R3]
            let system_join = multipass::<U8>()
                >> (map1.clone()
                    | map2.clone()
                    | map1.clone()
                    | map1.clone()
                    | map2.clone()
                    | map1.clone());
            net.push(Box::new(system_join))
        }
        7 => {
            // Groups: [L0] | [L1] | [L2] | [L3] | [R0] | [R1] | [R2,R3]
            let system_join = multipass::<U8>()
                >> (map1.clone()
                    | map1.clone()
                    | map1.clone()
                    | map1.clone()
                    | map1.clone()
                    | map1.clone()
                    | map2.clone());
            net.push(Box::new(system_join))
        }
        8 => {
            // Groups: [L0] | [L1] | [L2] | [L3] | [R0] | [R1] | [R2] | [R3]
            let system_join = multipass::<U8>()
                >> (map1.clone()
                    | map1.clone()
                    | map1.clone()
                    | map1.clone()
                    | map1.clone()
                    | map1.clone()
                    | map1.clone()
                    | map1.clone());
            net.push(Box::new(system_join))
        }
        _ => unreachable!(),
    };

    // Feed both sides into the joiner and expose the requested multi-channel output.
    net.pipe_all(left_filter_id, join_id);
    net.pipe_all(right_filter_id, join_id);

    net.pipe_output(join_id);

    node_handles
}

type Handles = (
    Vec<NodeHandles>,
    Vec<HashMap<NodeKey, InnerHandles>>,
    Vec<FilterHandles>,
);

fn prepare_handles(groups: &[GroupConfig]) -> Handles {
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

            let (excitement_snoop_front, excitement_snoop_backend) =
                snoop(node::ACTIVATION_SNOOP_CAPACITY);
            let (output_snoop_front, output_snoop_backend) = snoop(node::OUTPUT_SNOOP_CAPACITY);
            let siren_control = shared(0.0);
            let band_control = shared(0.0);
            let key_control = shared(0.0);

            group_handles.insert(
                key,
                InnerHandles {
                    excitement_snoop: excitement_snoop_backend,
                    output_snoop: output_snoop_backend,
                    siren_control: Var::new(&siren_control),
                    band_control: Var::new(&band_control),
                },
            );

            filter_handles.push(FilterHandles {
                control_a_b: Var::new(&key_control),
                control: Var::new(&siren_control),
                config: *node,
            });

            node_handles.push(NodeHandles {
                key,
                excitement_snoop: excitement_snoop_front,
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

    (node_handles, inner_handles, filter_handles)
}
