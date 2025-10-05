use common::instrument::{Config, GroupChanel, GroupConfig, NodeConfig};
use fundsp::{
    hacker32::prelude::*,
    typenum::{UInt, UTerm, Unsigned, B1},
};
use u_num_it::u_num_it;

type NodeType = Pipe<Constant<UInt<UTerm, B1>>, Sine<f32>>;

fn create_node(config: &NodeConfig) -> An<NodeType> {
    sine_hz(config.base_frequency as f32)
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
    let node_id = net.push(Box::new(node >> declick::<f32>()));
    net.connect_output(node_id, 0, channel);
}

fn mono_system(config: &Config, net: &mut Net) {
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

fn stereo_system(config: &Config, net: &mut Net) {
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

fn multi_channel_system(config: &Config, net: &mut Net, num_channels: usize) {
    todo!("multi_channel_system")
}

pub fn create_output_system(config: &Config, net: &mut Net, num_channels: usize) {
    match num_channels {
        1 => {
            mono_system(config, net);
        }
        2 => {
            stereo_system(config, net);
        }
        3.. => {
            multi_channel_system(config, net, num_channels);
        }
        0 => {
            panic!("Number of output channels cannot be zero");
        }
    }
}
