use std::{cell::RefCell, mem};

use common::instrument::{Config, GroupChannel, GroupConfig, NodeConfig};
use fundsp::{
    hacker32::prelude::*,
    typenum::{UInt, UTerm, B1},
};
use u_num_it::u_num_it;

use super::{siren::*, NodeHandles, NodeKey};

type S = f32;

type NodeType = Pipe<
    Pipe<
        Pipe<
            Pipe<
                Binop<FrameMul<UInt<UTerm, B1>>, Pipe<Constant<UInt<UTerm, B1>>, Sine<S>>, Siren>,
                Split<UInt<UInt<UTerm, B1>, B1>>,
            >,
            Binop<
                FrameAdd<UInt<UTerm, B1>>,
                Binop<
                    FrameAdd<UInt<UTerm, B1>>,
                    Unop<Resonator<S, UInt<UTerm, B1>>, FrameMulScalar<UInt<UTerm, B1>>>,
                    Unop<Resonator<S, UInt<UTerm, B1>>, FrameMulScalar<UInt<UTerm, B1>>>,
                >,
                Unop<Resonator<S, UInt<UTerm, B1>>, FrameMulScalar<UInt<UTerm, B1>>>,
            >,
        >,
        FixedSvf<S, HighpassMode<S>>,
    >,
    SnoopBackend,
>;

//
// pub struct NodeHandles {
//     pub key: NodeKey,
//     pub activation_snoop: Snoop,
//     pub output_snoop: Snoop,
//     pub siren_control: Shared,
//     pub band_control: Shared,
// }

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
        let (_, activation_snoop) = snoop(ACTIVATION_SNOOP_CAPACITY);
        let (_, output_snoop) = snoop(OUTPUT_SNOOP_CAPACITY);
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

const ACTIVATION_SNOOP_CAPACITY: usize = 16;
const OUTPUT_SNOOP_CAPACITY: usize = 256;

fn create_node(config: &NodeConfig, handles: InnerHandles, _a_coef: f32) -> An<NodeType> {
    let InnerHandles {
        activation_snoop: _activation_snoop,
        output_snoop: output_snoop_backend,
        siren_control,
        band_control: _band_control,
        ..
    } = handles;

    // Formant parameters
    let f1 = 730.0;
    let f2 = 1090.0;
    let f3 = 2440.0;

    // Bandwidths (in Hz) for more natural sound
    let bw1 = 100.0;
    let bw2 = 120.0;
    let bw3 = 150.0;

    let siren = siren(siren_control);

    // Source
    let source = (sine_hz::<S>(config.base_frequency as S) * siren) >> split::<U3>();

    // Create resonator formants
    let formants = source
        >> ((resonator_hz(f1, bw1) * 1.0)
            + (resonator_hz(f2, bw2) * 0.8)
            + (resonator_hz(f3, bw3) * 0.6));

    formants >> highpass_hz(80.0, 1.0) >> output_snoop_backend
}

fn create_group_node<K>(
    config: &GroupConfig,
    group_handles: Vec<InnerHandles>,
) -> An<MultiBus<K, NodeType>>
where
    K: Size<f32> + Size<NodeType>,
{
    let nodes = config.nodes.clone();
    let a_coef = config.a_coef;
    let handles_cell = RefCell::new(group_handles);
    busi::<K, _, _>(move |i| {
        let handle = mem::take(&mut handles_cell.borrow_mut()[i as usize]);
        create_node(&nodes[i as usize], handle, a_coef)
    })
}

fn create_channel_system<G, K>(
    groups: &[GroupConfig],
    channel: usize,
    net: &mut Net,
) -> Vec<NodeHandles>
where
    G: Size<f32> + Size<NodeType>,
    K: Size<f32> + Size<NodeType>,
{
    let mut node_handles = Vec::<NodeHandles>::new();
    let mut inner_handles = Vec::<Vec<InnerHandles>>::new();

    for (gi, group) in groups.iter().enumerate() {
        let mut group_handles = Vec::<InnerHandles>::new();
        for (ki, _node) in group.nodes.iter().enumerate() {
            let key = NodeKey(gi as u8, ki as u8);

            let (activation_snoop_front, activation_snoop_backend) =
                snoop(ACTIVATION_SNOOP_CAPACITY);
            let (output_snoop_front, output_snoop_backend) = snoop(OUTPUT_SNOOP_CAPACITY);
            let siren_control = shared(group.a_coef);
            let band_control = shared(0.0);

            group_handles.push(InnerHandles {
                key,
                activation_snoop: activation_snoop_backend,
                output_snoop: output_snoop_backend,
                siren_control: Var::new(&siren_control),
                band_control: Var::new(&band_control),
            });

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
        create_group_node::<K>(&groups[i as usize], group_handles)
    });
    let node_id = net.push(Box::new(node >> dcblock::<S>() >> declick::<S>()));
    net.connect_output(node_id, 0, channel);
    node_handles
}

pub fn mono_system(config: &Config, net: &mut Net) -> Vec<NodeHandles> {
    let nodes_count_per_group = config.num_nodes_per_group();
    let groups = config.0.as_slice();

    one_channel_subsystem(groups, nodes_count_per_group, GroupChannel::Left, net)
}

fn one_channel_subsystem(
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
