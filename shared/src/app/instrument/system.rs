use super::{
    keyboard::{Button, Track},
    Config, Node,
};
use fundsp::hacker32::*;
use hecs::{Entity, World};

pub struct System {
    pub net: Net32,
    pub net_be: NetBackend32,
    pub size: usize,
    pub entities: Vec<Entity>,
    pub nodes: Vec<NodeId>,
    pub amps: Vec<Shared<f32>>,
    pub tunes: Vec<Shared<f32>>,
}

impl System {
    fn spawn_nodes(world: &mut World) -> Vec<Entity> {
        let mut nodes = world
            .query::<&Button>()
            .iter()
            .map(|(_, b)| {
                let mut query = world.query_one::<&Track>(b.track).unwrap();
                let track = query.get().unwrap();
                (track.freq, b.f_n, if track.left_hand { -1 } else { 1 })
            })
            .collect::<Vec<_>>();

        nodes.sort_by(|a, b| a.1.cmp(&b.1));
        nodes.reverse();

        let nodes = nodes
            .into_iter()
            .map(|(freq, f_n, pan)| Node::spawn(world, freq, f_n, pan))
            .collect::<Vec<_>>();

        nodes
    }

    pub fn spawn(world: &mut World, config: &Config) -> Self {
        let mut net = Net32::new(1, config.channels);
        let entities: Vec<Entity> = System::spawn_nodes(world);
        let size = entities.len();
        let nodes_data = entities
            .iter()
            .map(|e| *world.get::<&Node>(*e).expect("node for entity"))
            .collect::<Vec<_>>();

        let mut amps = Vec::with_capacity(size);
        let mut tunes = Vec::with_capacity(size);
        let mut nodes = Vec::with_capacity(size);

        // for Node {
        //     freq: (base_freq, max_freq),
        //     ..
        // } in nodes_data.iter()
        // {
        //     let c = sine_hz(*base_freq);
            let c = sine_hz(440.0);

        //     let a = shared(1.0);

        //     let t = shared(0.0);

        //     let d = max_freq - base_freq;

        //     let node = (c + d * var(&t)) * var(&a);

            let id = net.push(Box::new(c));

            nodes.push(id);

            net.connect_output(id, 0, 0);
        //     amps.push(a);
        //     tunes.push(t);
        // }

        // let mut subnet = Net32::new(size, config.channels);

        // let mut left_join_id = Some(subnet.push(Box::new(join::<U2>())));
        // let mut right_join_id = if config.channels > 1 {
        //     Some(subnet.push(Box::new(join::<U2>())))
        // } else {
        //     None
        // };

        // if let Some(left) = left_join_id {
        //     subnet.connect_output(left, 0, 0);
        // }
        // if let Some(right) = right_join_id {
        //     subnet.connect_output(right, 0, 1);
        // }

        // let (left, right) = nodes_data.iter().fold((vec![], vec![]), |mut acc, node| {
        //     if node.pan > 0 && config.channels > 1 {
        //         acc.1.push(node)
        //     } else {
        //         acc.0.push(node)
        //     }
        //     acc
        // });

        // let mut it = left.iter().peekable();

        // while let Some(Node { f_n, .. }) = it.next() {
        //     let left = left_join_id.take().unwrap();
        //     if it.peek().is_some() {
        //         subnet.connect_input(f_n - 1, left, 0);

        //         let next_left_join_id = subnet.push(Box::new(join::<U2>()));
        //         subnet.connect(next_left_join_id, 0, left, 1);
        //         left_join_id = Some(next_left_join_id);
        //     } else {
        //         subnet.connect_input(f_n - 1, left, 1);
        //     }
        // }

        // let mut it = right.iter().peekable();

        // while let Some(Node { f_n, .. }) = it.next() {
        //     let right = right_join_id.take().unwrap();
        //     if it.peek().is_some() {
        //         subnet.connect_input(f_n - 1, right, 0);

        //         let next_right_join_id = subnet.push(Box::new(join::<U2>()));
        //         subnet.connect(next_right_join_id, 0, right, 1);
        //         right_join_id = Some(next_right_join_id);
        //     } else {
        //         subnet.connect_input(f_n - 1, right, 1);
        //     }
        // }

        // let subnet_id = net.push(Box::new(subnet));

        // for (Node { f_n, .. }, id) in nodes_data.iter().zip(nodes.iter()) {
        //     net.connect(*id, 0, subnet_id, f_n - 1);
        // }

        // for n in 0..config.channels {
        //     net.connect_output(subnet_id, n, n);
        // }



        net.set_sample_rate(config.sample_rate_hz);

        net.check();

        let mut net_be = net.backend();

        net_be.allocate();

        Self {
            net,
            net_be,
            size,
            entities,
            amps,
            tunes,
            nodes,
        }
    }

    pub fn get_nodes(&self, world: &World) -> Vec<Node> {
        self.entities
            .iter()
            .map(|e| *world.get::<&Node>(*e).expect("node for entity"))
            .collect()
    }
}
