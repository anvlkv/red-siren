use common::instrument::{Config, GroupChanel, GroupConfig, NodeConfig};
use fundsp::{
    hacker32::prelude::*,
    typenum::{UInt, UTerm, Unsigned, B1},
};
use u_num_it::u_num_it;

type S = f32;

type NodeType = Pipe<
    Unop<
        Pipe<
            Pipe<Pipe<Constant<UInt<UTerm, B1>>, Sine<S>>, Split<UInt<UInt<UTerm, B1>, B1>>>,
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
        FrameMulScalar<UInt<UTerm, B1>>,
    >,
    FixedSvf<S, HighpassMode<S>>,
>;

fn create_node(config: &NodeConfig) -> An<NodeType> {
    // Formant parameters
    let f1 = 730.0;
    let f2 = 1090.0;
    let f3 = 2440.0;

    // Bandwidths (in Hz) for more natural sound
    let bw1 = 100.0;
    let bw2 = 120.0;
    let bw3 = 150.0;

    // Source
    let source = sine_hz::<S>(config.base_frequency as S) >> split::<U3>();

    // Create resonator formants
    let formants = source
        >> ((resonator_hz(f1, bw1) * 1.0)
            + (resonator_hz(f2, bw2) * 0.8)
            + (resonator_hz(f3, bw3) * 0.6));

    let vowel = formants * 0.25;
    vowel >> highpass_hz(80.0, 1.0)
}

fn create_group_node<K>(config: &GroupConfig) -> An<MultiBus<K, NodeType>>
where
    K: Size<f32> + Size<NodeType>,
{
    let nodes = config.nodes.as_slice();
    busi::<K, _, _>(|i| create_node(&nodes[i as usize]))
}

fn create_channel_system<G, K>(groups: &[GroupConfig], channel: usize, net: &mut Net)
where
    G: Size<f32> + Size<NodeType>,
    K: Size<f32> + Size<NodeType>,
{
    let node = busi::<G, _, _>(|i| create_group_node::<K>(&groups[i as usize]));
    let node_id = net.push(Box::new(node >> dcblock::<S>() >> declick::<S>()));
    net.connect_output(node_id, 0, channel);
}

pub fn mono_system(config: &Config, net: &mut Net) {
    let nodes_count_per_group = config.num_nodes_per_group();
    let groups_count = config.num_groups();

    u_num_it!(
        [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71],
        match groups_count {
            U => {
                type GNum = NumType;
                let groups = config.0.as_slice();
                u_num_it!(
                    [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71],
                    match nodes_count_per_group {
                        U => {
                            type KNum = NumType;

                            create_channel_system::<GNum, KNum>(groups, 0, net);
                            log::debug!("created mono channel system");
                        }
                        _ => {
                            panic!("unexpected number of nodes")
                        }
                    }
                );
            }
            _ => {
                panic!("unexpected number of groups")
            }
        }
    );
}

pub fn stereo_system(config: &Config, net: &mut Net) {
    let nodes_count_per_group = config.num_nodes_per_group();
    let groups_count_left = config.num_groups_left();
    let groups_count_right = config.num_groups_right();

    u_num_it!(
        1..=36,
        match groups_count_left {
            U => {
                type GNum = NumType;
                let groups = (0..GNum::to_usize())
                    .filter_map(|gi| config.group_nth_channel(GroupChanel::Left, gi))
                    .cloned()
                    .collect::<Vec<_>>();
                u_num_it!(
                    [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71],
                    match nodes_count_per_group {
                        U => {
                            type KNum = NumType;

                            create_channel_system::<GNum, KNum>(
                                groups.as_slice(),
                                GroupChanel::Left as usize,
                                net,
                            );
                            log::debug!("created stereo [left] channel system");
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
    u_num_it!(
        1..=36,
        match groups_count_right {
            U => {
                type GNum = NumType;
                let groups = (0..GNum::to_usize())
                    .filter_map(|gi| config.group_nth_channel(GroupChanel::Right, gi))
                    .cloned()
                    .collect::<Vec<_>>();
                u_num_it!(
                    [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71],
                    match nodes_count_per_group {
                        U => {
                            type KNum = NumType;

                            create_channel_system::<GNum, KNum>(
                                groups.as_slice(),
                                GroupChanel::Right as usize,
                                net,
                            );

                            log::debug!("created stereo [right] channel system");
                        }
                        _ => {
                            panic!("unexpected number of nodes");
                        }
                    }
                );
            }
            _ => {
                panic!("unexpected number of groups in right channel");
            }
        }
    );
}

pub fn multi_channel_system(_config: &Config, _net: &mut Net, _num_channels: usize) {
    todo!("multi_channel_system")
}
