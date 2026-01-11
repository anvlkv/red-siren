use std::{cell::RefCell, collections::HashMap, mem};

use common::{
    instrument::{GroupChannel, GroupConfig},
    NodeKey,
};
use fundsp::prelude::*;
use u_num_it::u_num_it;

use super::{
    filter::{FilterHandles, FilterType},
    node::NodeType,
    throw_catch::{ThrowCatchCatch, ThrowCatchThrow},
    InnerHandles,
};

use crate::system::values::FineTunedValues;

pub fn add_one_channel_subsystem<S: Real + Float + 'static>(
    channel_groups: &[GroupConfig],
    (group_handles, filter_handles): (Vec<HashMap<NodeKey, InnerHandles>>, Vec<FilterHandles>),
    nodes_count_per_group: usize,
    channel: GroupChannel,
    (throw_x, catch_x): (An<ThrowCatchThrow>, An<ThrowCatchCatch>),
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
                    [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71,],
                    match nodes_count_per_group {
                        U => {
                            type KNum = NumType;

                            let id = add_channel_system::<GNum, KNum, S>(
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

    let cross = split::<U2>()
        >> (pass()
            | (catch_x
                * (constant(0.1)
                    >> sine::<S>().phase(match channel {
                        GroupChannel::Left => 0.25,
                        GroupChannel::Right => 0.0,
                    })))
            | throw_x)
        >> map(|frame: &Frame<f32, U2>| {
            let main = frame[0];
            let x = frame[1] * 0.1;
            main + x.abs() * main.signum()
        });
    let cross_id = net.push(Box::new(cross));
    net.pipe_all(id, cross_id);

    u_num_it!(
        1..=71,
        match channel_filters_count {
            U => {
                type FNum = NumType;
                id = add_channel_filter::<FNum, S>(filter_handles, cross_id, net, values);
            }
            _ => {
                panic!("unexpected number of filters");
            }
        }
    );

    id
}

fn add_channel_system<G, K, S>(
    groups: &[GroupConfig],
    handles: Vec<HashMap<NodeKey, InnerHandles>>,
    net: &mut Net,
    values: &FineTunedValues,
) -> NodeId
where
    G: Size<S> + Size<NodeType<S>>,
    K: Size<S> + Size<NodeType<S>>,
    S: Real + Float + 'static,
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
            super::group::create_group_node::<K, S>(&groups[i as usize], group_handles, &values)
        }
    });
    net.push(Box::new(node))
}

fn add_channel_filter<F, S>(
    filter_handles: Vec<FilterHandles>,
    src_id: NodeId,
    net: &mut Net,
    values: &FineTunedValues,
) -> NodeId
where
    F: Size<S> + Size<FilterType<S>>,
    S: Real + Float + 'static,
{
    log::info!(
        "Creating channel filter with {} filters",
        filter_handles.len(),
    );

    let band_controls_value = constant(0.0)
        >> (pipei::<F, _, _>({
            let band_controls = RefCell::new(
                filter_handles
                    .iter()
                    .map(|h| h.control.clone())
                    .collect::<Vec<_>>(),
            );
            move |_| {
                let mut band_controls = band_controls.borrow_mut();
                pass() + var(&band_controls.pop().unwrap())
            }
        }) | constant(F::USIZE as f32))
        >> super::div::div::<S>();

    let ab_controls_value = constant(0.0)
        >> (pipei::<F, _, _>({
            let ab_controls = RefCell::new(
                filter_handles
                    .iter()
                    .map(|h| h.control_a_b.clone())
                    .collect::<Vec<_>>(),
            );
            move |_| {
                let mut ab_controls = ab_controls.borrow_mut();
                pass() + var(&ab_controls.pop().unwrap())
            }
        }) | constant(F::USIZE as f32))
        >> super::div::div::<S>();

    let gain = S::from_f64(((1.0 / F::USIZE as f64) + 1.0).powi(F::I32));

    let panner_node = (pass()
        | ((band_controls_value + (ab_controls_value.clone() >> mul(-1.5))) >> mul(0.25)))
        >> panner();

    let filter_channel = pinkpass::<S>()
        >> split::<U2>()
        >> pipei::<F, _, _>({
            let values = values.clone();
            move |i| {
                super::filter::create_filter::<S>(filter_handles[i as usize].clone(), &values, gain)
            }
        })
        >> (pass() + pass());

    let composite_channel = panner_node
        >> (filter_channel | pass())
        >> reverb4_stereo(
            17.5,
            (1.0 + (1.0 / F::USIZE as f64)) * (F::USIZE as f64).powf(-0.1),
        )
        >> ((pass() + (pass() * -0.15))
            * (constant(1.0)
                + (((constant(1.0) - ab_controls_value) | constant(1.0 / F::USIZE as f32))
                    >> super::div::div::<S>())))
        >> dcblock::<S>();

    let filter_id = net.push(Box::new(composite_channel));
    net.pipe_all(src_id, filter_id);

    filter_id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        output::throw_catch,
        system::output::{
            prepare_handles, split_filter_handles_lr, split_group_handles_lr, split_groups_lr,
        },
    };
    use common::instrument::config_test_cases;
    use insta_fun::prelude::*;

    #[test]
    fn one_group_one_filter() {
        let mut svg_config = SvgChartConfigBuilder::default();
        svg_config.show_grid(true);
        svg_config.preserve_aspect_ratio(SvgPreserveAspectRatio::scale_to_fit());

        let mut snapshot_config = SnapshotConfigBuilder::default();
        snapshot_config.num_samples(2000);
        snapshot_config.warm_up(WarmUp::Seconds(0.5));
        snapshot_config.allow_abnormal_samples(true);

        #[cfg(feature = "editor")]
        let values = FineTunedValues::new(&FineTunedSharedValues::default());
        #[cfg(not(feature = "editor"))]
        let values = FineTunedValues::new();

        for (config, layout) in config_test_cases() {
            for group in config.0 {
                let g_key = group.nodes.first().unwrap().key.group();

                let mut net = Net::new(0, 1);
                let groups = [group];
                let (node_handles, group_handles, filter_handles) =
                    prepare_handles(&groups, config.1);

                node_handles.iter().for_each(|n| {
                    n.siren_control.set_value((0.25, 0.1));
                    n.band_control.set_value(0.1);
                });

                let id = add_one_channel_subsystem::<f32>(
                    &groups,
                    (group_handles, filter_handles),
                    layout.num_keys_per_group.get() as usize,
                    GroupChannel::Left,
                    throw_catch::throw_catch(7),
                    &mut net,
                    &values,
                );

                net.pipe_output(id);

                let svg_config = svg_config
                    .clone()
                    .chart_title(format!(
                        "config_test_case_group_and_filter_{g_key}_{}x{}_{:?}",
                        layout.space.x, layout.space.y, layout.scale
                    ))
                    .build()
                    .unwrap();

                let snapshot_config = snapshot_config
                    .clone()
                    .output_mode(svg_config)
                    .build()
                    .unwrap();

                assert_audio_unit_snapshot!(net, snapshot_config);
            }
        }
    }

    #[test]
    fn one_channel_direct() {
        let mut svg_config_bldr = SvgChartConfigBuilder::default();
        svg_config_bldr.show_grid(true);
        svg_config_bldr.preserve_aspect_ratio(SvgPreserveAspectRatio::scale_to_fit());

        let mut snapshot_config_bldr = SnapshotConfigBuilder::default();
        snapshot_config_bldr.num_samples(2000);
        snapshot_config_bldr.warm_up(WarmUp::Seconds(0.5));
        snapshot_config_bldr.allow_abnormal_samples(true);

        #[cfg(feature = "editor")]
        let values = FineTunedValues::new(&FineTunedSharedValues::default());
        #[cfg(not(feature = "editor"))]
        let values = FineTunedValues::new();

        for (config, layout) in config_test_cases() {
            let (node_handles, group_handles, filter_handles) =
                prepare_handles(&config.0, config.1);

            node_handles.iter().for_each(|n| {
                n.siren_control.set_value((0.25, 0.1));
                n.band_control.set_value(0.1);
            });

            let nodes_count_per_group = config.num_nodes_per_group();
            let (left_groups, right_groups) = split_groups_lr(&config);

            let (left_group_handles, right_group_handles) =
                split_group_handles_lr(group_handles, &config);

            let (left_filter_handles, right_filter_handles) =
                split_filter_handles_lr(filter_handles, &config);

            let mut left_net = Net::new(0, 1);

            let id = add_one_channel_subsystem::<f32>(
                &left_groups,
                (left_group_handles, left_filter_handles),
                nodes_count_per_group,
                GroupChannel::Left,
                throw_catch::throw_catch(7),
                &mut left_net,
                &values,
            );

            left_net.pipe_output(id);

            let svg_config = svg_config_bldr
                .clone()
                .chart_title(format!(
                    "config_test_case_left_channel_groups_direct_{}x{}_{:?}",
                    layout.space.x, layout.space.y, layout.scale
                ))
                .build()
                .unwrap();

            let snapshot_config = snapshot_config_bldr
                .clone()
                .output_mode(svg_config)
                .build()
                .unwrap();

            assert_audio_unit_snapshot!(left_net, snapshot_config);

            let mut right_net = Net::new(0, 1);

            let id = add_one_channel_subsystem::<f32>(
                &right_groups,
                (right_group_handles, right_filter_handles),
                nodes_count_per_group,
                GroupChannel::Right,
                throw_catch::throw_catch(7),
                &mut right_net,
                &values,
            );

            right_net.pipe_output(id);

            let svg_config = svg_config_bldr
                .clone()
                .chart_title(format!(
                    "config_test_case_right_channel_groups_direct_{}x{}_{:?}",
                    layout.space.x, layout.space.y, layout.scale
                ))
                .build()
                .unwrap();

            let snapshot_config = snapshot_config_bldr
                .clone()
                .output_mode(svg_config)
                .build()
                .unwrap();

            assert_audio_unit_snapshot!(right_net, snapshot_config);
        }
    }

    #[test]
    fn one_channel_cross() {
        let mut svg_config_bldr = SvgChartConfigBuilder::default();
        svg_config_bldr.show_grid(true);
        svg_config_bldr.preserve_aspect_ratio(SvgPreserveAspectRatio::scale_to_fit());

        let mut snapshot_config_bldr = SnapshotConfigBuilder::default();
        snapshot_config_bldr.num_samples(2000);
        snapshot_config_bldr.warm_up(WarmUp::Seconds(0.5));
        snapshot_config_bldr.allow_abnormal_samples(true);

        #[cfg(feature = "editor")]
        let values = FineTunedValues::new(&FineTunedSharedValues::default());
        #[cfg(not(feature = "editor"))]
        let values = FineTunedValues::new();

        for (config, layout) in config_test_cases() {
            let (node_handles, group_handles, filter_handles) =
                prepare_handles(&config.0, config.1);

            node_handles.iter().for_each(|n| {
                n.siren_control.set_value((0.25, 0.1));
                n.band_control.set_value(0.1);
            });

            let nodes_count_per_group = config.num_nodes_per_group();
            let (left_groups, right_groups) = split_groups_lr(&config);

            let (left_group_handles, right_group_handles) =
                split_group_handles_lr(group_handles, &config);

            let (left_filter_handles, right_filter_handles) =
                split_filter_handles_lr(filter_handles, &config);

            let mut left_net = Net::new(0, 1);

            let id = add_one_channel_subsystem::<f32>(
                &left_groups,
                (left_group_handles, right_filter_handles),
                nodes_count_per_group,
                GroupChannel::Left,
                throw_catch::throw_catch(7),
                &mut left_net,
                &values,
            );

            left_net.pipe_output(id);

            let svg_config = svg_config_bldr
                .clone()
                .chart_title(format!(
                    "config_test_case_left_channel_groups_cross_{}x{}_{:?}",
                    layout.space.x, layout.space.y, layout.scale
                ))
                .build()
                .unwrap();

            let snapshot_config = snapshot_config_bldr
                .clone()
                .output_mode(svg_config)
                .build()
                .unwrap();

            assert_audio_unit_snapshot!(left_net, snapshot_config);

            let mut right_net = Net::new(0, 1);

            let id = add_one_channel_subsystem::<f32>(
                &right_groups,
                (right_group_handles, left_filter_handles),
                nodes_count_per_group,
                GroupChannel::Right,
                throw_catch::throw_catch(7),
                &mut right_net,
                &values,
            );

            right_net.pipe_output(id);

            let svg_config = svg_config_bldr
                .clone()
                .chart_title(format!(
                    "config_test_case_right_channel_groups_cross_{}x{}_{:?}",
                    layout.space.x, layout.space.y, layout.scale
                ))
                .build()
                .unwrap();

            let snapshot_config = snapshot_config_bldr
                .clone()
                .output_mode(svg_config)
                .build()
                .unwrap();

            assert_audio_unit_snapshot!(right_net, snapshot_config);
        }
    }
}
