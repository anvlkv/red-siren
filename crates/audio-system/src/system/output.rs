mod abs;
mod channel;
mod crossfade;
pub(crate) mod db_lin;
mod div;
mod filter;
mod formant;
mod group;
mod metro;
mod node;
mod pow;
mod siren;
mod throw_catch;

use std::collections::HashMap;

use channel::add_one_channel_subsystem;
use common::{
    instrument::{Config, GroupChannel, GroupConfig, Scale},
    NodeKey,
};
use fundsp::prelude::*;

use super::NodeHandles;
use crate::{
    output::filter::FilterHandles, system::values::FineTunedValues, util::S, ExcitementControl,
};

#[derive(Clone)]
pub(super) struct InnerHandles {
    excitement_snoop: An<SnoopBackend>,
    secondary_excitement_snoop: An<SnoopBackend>,
    output_snoop: An<SnoopBackend>,
    siren_control: ExcitementControl,
    band_control: Shared,
    key_control: Shared,
    siren_signum: Constant<U1>,
}

impl Default for InnerHandles {
    fn default() -> Self {
        let (_, excitement_snoop) = snoop(node::ACTIVATION_SNOOP_CAPACITY);
        let (_, secondary_excitement_snoop) = snoop(node::ACTIVATION_SNOOP_CAPACITY);
        let (_, output_snoop) = snoop(node::OUTPUT_SNOOP_CAPACITY);
        let siren_control = ExcitementControl::default();
        let band_control = shared(0.0);
        let key_control = shared(0.0);
        let siren_signum = Constant::new([1.0].into());

        Self {
            excitement_snoop,
            secondary_excitement_snoop,
            output_snoop,
            siren_control,
            band_control,
            key_control,
            siren_signum,
        }
    }
}

pub fn mono_system(config: &Config, net: &mut Net, values: &FineTunedValues) -> Vec<NodeHandles> {
    let nodes_count_per_group = config.num_nodes_per_group();
    let groups = config.0.as_slice();

    let (node_handles, group_handles, filter_handles) = prepare_handles(groups, config.1);

    let groups_count = groups.len();

    let (throw_x, catch_x) = throw_catch::throw_catch(7);

    let id = add_one_channel_subsystem(
        groups,
        (group_handles, filter_handles),
        nodes_count_per_group,
        GroupChannel::Left,
        (throw_x, catch_x),
        net,
        values,
    );

    let system_filter =
        split::<U2>() >> (chorus(groups_count as u64, 0.05, 0.025, 17.0) | pass()) >> join::<U2>();

    let system_filter_id = net.push(Box::new(system_filter));

    net.pipe_all(id, system_filter_id);

    net.pipe_output(system_filter_id);

    node_handles
}

#[allow(clippy::unnecessary_cast)]
pub fn stereo_system(config: &Config, net: &mut Net, values: &FineTunedValues) -> Vec<NodeHandles> {
    log::info!(
        "Creating stereo output system with {} total groups",
        config.num_groups()
    );

    let (node_handles, group_handles, filter_handles) = prepare_handles(&config.0, config.1);

    let nodes_count_per_group = config.num_nodes_per_group();

    let (left_groups, right_groups) = split_groups_lr(config);

    let (left_group_handles, right_group_handles) = split_group_handles_lr(group_handles, config);

    let (throw_l, catch_r) = throw_catch::throw_catch(3);
    let (throw_r, catch_l) = throw_catch::throw_catch(3);

    let (left_filter_handles, right_filter_handles) =
        split_filter_handles_lr(filter_handles, config);

    let left_id = add_one_channel_subsystem(
        left_groups.as_slice(),
        (left_group_handles, right_filter_handles),
        nodes_count_per_group,
        GroupChannel::Left,
        (throw_l, catch_l),
        net,
        values,
    );
    let right_id = add_one_channel_subsystem(
        right_groups.as_slice(),
        (right_group_handles, left_filter_handles),
        nodes_count_per_group,
        GroupChannel::Right,
        (throw_r, catch_r),
        net,
        values,
    );

    let system_filter = |seed: u64| {
        split::<U2>()
        >> (chorus(seed, 0.05, 0.025, 17.0) & (chorus(seed*2, 0.05, 0.025, 17.0) * -1.0) | pass())
        >> (pass() | pass())
        >> (pan(-0.15) | pan(0.85))
        // tt^ | tm | um | ut^
        // tt^ | um | tm | ut^
        >> (pass() | reverse::<U2>() | pass())
    };

    let groups_count_left = config.num_groups_left();
    let groups_count_right = config.num_groups_right();
    let left_filter_id = net.push(Box::new(system_filter(groups_count_left as u64)));
    let right_filter_id = net.push(Box::new(system_filter(groups_count_right as u64)));

    net.pipe_all(left_id, left_filter_id);
    net.pipe_all(right_id, right_filter_id);

    let mapper = map(|frame: &Frame<f32, U4>| {
        let treated = frame[0]; // tt^
        let untreated_mixin = frame[1]; // um - from opposite channel
        let treated_mixin = frame[2]; // tm - from opposite channel
        let untreated = frame[3]; // ut^

        // Weighted own blend
        let own = 0.65 * treated + 0.35 * untreated;

        // Pan already attenuates um, tm. Mild additional reduction.
        let raw_season = 0.15 * treated_mixin + 0.10 * untreated_mixin;
        let season_factor = 1.0 / (1.0 + 2.0 * own.abs());
        let season = raw_season * season_factor;

        own + season
    });

    let min_hz = config.min_frequency_hz();

    let system_join = multipass::<U8>()
        // Initial: 0: L_tt^ | 1: L_um | 2: L_tm | 3: L_ut^ | 4: R_tt^ | 5: R_um | 6: R_tm | 7: R_ut^
        >> An(Map::new(|frame: &Frame<f32, U8>| -> Frame<f32, U8> {
            [frame[0], frame[5], frame[6], frame[3], frame[4], frame[1], frame[2], frame[7]].into()
        }, Routing::Reverse))
        // Final grouping: [Left mapper frame] | [Right mapper frame]
        >> (mapper.clone() | mapper.clone())
        >> (dcblock_hz::<S>(min_hz as S * 0.1) | dcblock_hz::<S>(min_hz as S * 0.1));

    let join_id = net.push(Box::new(system_join));

    net.pipe_all(left_filter_id, join_id);
    net.pipe_all(right_filter_id, join_id);

    net.pipe_output(join_id);

    node_handles
}

pub fn multi_channel_system(
    config: &Config,
    net: &mut Net,
    _num_channels: usize,
    values: &FineTunedValues,
) -> Vec<NodeHandles> {
    stereo_system(config, net, values)
}

pub(super) type Handles = (
    Vec<NodeHandles>,
    Vec<HashMap<NodeKey, InnerHandles>>,
    Vec<FilterHandles>,
);

pub(super) fn prepare_handles(groups: &[GroupConfig], scale: Scale) -> Handles {
    let mut node_handles = Vec::<NodeHandles>::new();
    let mut inner_handles = Vec::<HashMap<NodeKey, InnerHandles>>::new();
    let mut filter_handles = Vec::<FilterHandles>::new();
    let siren_signum = Constant::new(
        match scale {
            Scale::Yo => [-1.0],
            Scale::In => [1.0],
        }
        .into(),
    );

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
            let (secondary_excitement_snoop_front, secondary_excitement_snoop_backend) =
                snoop(node::ACTIVATION_SNOOP_CAPACITY);
            let (output_snoop_front, output_snoop_backend) = snoop(node::OUTPUT_SNOOP_CAPACITY);
            let siren_control = ExcitementControl::default();
            let band_control = shared(0.0);
            let key_control = shared(0.0);

            group_handles.insert(
                key,
                InnerHandles {
                    excitement_snoop: excitement_snoop_backend,
                    secondary_excitement_snoop: secondary_excitement_snoop_backend,
                    output_snoop: output_snoop_backend,
                    siren_control: siren_control.clone(),
                    band_control: band_control.clone(),
                    key_control: key_control.clone(),
                    siren_signum: siren_signum.clone(),
                },
            );

            filter_handles.push(FilterHandles {
                control_a_b: key_control.clone(),
                control: band_control.clone(),
                secondary_xct: siren_control.secondary.clone(),
                config: *node,
            });

            node_handles.push(NodeHandles {
                key,
                excitement_snoop: excitement_snoop_front,
                secondary_excitement_snoop: secondary_excitement_snoop_front,
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

pub(super) fn split_groups_lr(config: &Config) -> (Vec<GroupConfig>, Vec<GroupConfig>) {
    fn select_groups(channel: GroupChannel, config: &Config) -> Vec<GroupConfig> {
        let count = match channel {
            GroupChannel::Left => config.num_groups_left(),
            GroupChannel::Right => config.num_groups_right(),
        };
        (0..count)
            .filter_map(|gi| config.group_nth_channel(channel, gi))
            .cloned()
            .collect::<Vec<_>>()
    }

    (
        select_groups(GroupChannel::Left, config),
        select_groups(GroupChannel::Right, config),
    )
}

pub(super) type GroupKeyMap = HashMap<NodeKey, InnerHandles>;

pub(super) fn split_group_handles_lr(
    group_handles: impl IntoIterator<Item = GroupKeyMap>,
    config: &Config,
) -> (Vec<GroupKeyMap>, Vec<GroupKeyMap>) {
    group_handles.into_iter().fold(
        (Vec::<GroupKeyMap>::new(), Vec::<GroupKeyMap>::new()),
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
    )
}

pub(super) fn split_filter_handles_lr(
    filter_handles: impl IntoIterator<Item = FilterHandles>,
    config: &Config,
) -> (Vec<FilterHandles>, Vec<FilterHandles>) {
    filter_handles.into_iter().fold(
        (Vec::<FilterHandles>::new(), Vec::<FilterHandles>::new()),
        |(mut left, mut right), handle| {
            match config.channel_of_key(&handle.config.key) {
                Some(GroupChannel::Left) => left.push(handle),
                Some(GroupChannel::Right) => right.push(handle),
                _ => log::error!("Invalid group channel"),
            }

            (left, right)
        },
    )
}
