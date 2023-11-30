use super::{
    keyboard::{Button, Track},
    Config, Node,
};
use fundsp::hacker32::*;
use hecs::{Entity, World};

pub struct System {
    pub net_be: BigBlockAdapter32,
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

        // let input_unit = pass() >> dcblock() >> input_branch;

        // let input_id = net.push(Box::new(input_unit));
        // net.pipe_input(input_id);

        for Node {
            freq: (base_freq, max_freq),
            f_n,
            ..
        } in nodes_data.iter()
        {
            let c = sine_hz(*base_freq); //envelope(|t| sin_hz(base_freq.clone(), t));

            let a = shared(1.0);

            let t = shared(0.0);

            let d = sine_hz(max_freq - base_freq);

            let max_freq = *max_freq;

            let node = (biquad(0.0, 0.0, 1.0 / *f_n as f32, 2.0, 1.0) * var(&a)) 
                >> hold_hz(10.0 * (*f_n as f32), 0.25)
                >> clip()
                >> follow(0.2)
                >> pluck(max_freq, 0.17, 0.13)
                >> clip_to(0.0, (size - f_n + 2) as f32);

            let mut node = node * (c + d * var(&t));

            log::debug!("created node: {}", node.display());

            let id = net.push(Box::new(node));

            nodes.push(id);
            amps.push(a);
            tunes.push(t);
        }

        let mut subnet = Net32::new(size, config.channels);

        let lp = nodes_data.last().map(|n| n.freq.1).unwrap_or_default();
        let hp = nodes_data.first().map(|n| n.freq.0).unwrap_or_default();
        let subs_join = join::<U2>()
         >> declick_s(1.0) 
         >> split::<U3>() 
         >> (pinkpass() | lowpass_hz(lp, 1.0) | highpass_hz(hp, 1.0)) 
         >> (pass() | highpass_hz(hp, 0.25) | lowpass_hz(lp, 0.25)) 
         >> join::<U3>();
        let mut left_join_id = Some(subnet.push(Box::new(subs_join.clone())));
        let mut right_join_id = if config.channels > 1 {
            Some(subnet.push(Box::new(subs_join.clone())))
        } else {
            None
        };

        if let Some(left) = left_join_id {
            subnet.connect_output(left, 0, 0);
        }
        if let Some(right) = right_join_id {
            subnet.connect_output(right, 0, 1);
        }

        let (left, right) = nodes_data.iter().fold((vec![], vec![]), |mut acc, node| {
            if node.pan > 0 && config.channels > 1 {
                acc.1.push(node)
            } else {
                acc.0.push(node)
            }
            acc
        });

        let mut it = left.iter().peekable();

        while let Some(Node { f_n, .. }) = it.next() {
            let left = left_join_id.take().unwrap();
            if it.peek().is_some() {
                subnet.connect_input(f_n - 1, left, 0);

                let next_left_join_id = subnet.push(Box::new(join::<U2>()));
                subnet.connect(next_left_join_id, 0, left, 1);
                left_join_id = Some(next_left_join_id);
            } else {
                subnet.connect_input(f_n - 1, left, 1);
            }
        }

        let mut it = right.iter().peekable();

        while let Some(Node { f_n, .. }) = it.next() {
            let right = right_join_id.take().unwrap();
            if it.peek().is_some() {
                subnet.connect_input(f_n - 1, right, 0);

                let next_right_join_id = subnet.push(Box::new(join::<U2>()));
                subnet.connect(next_right_join_id, 0, right, 1);
                right_join_id = Some(next_right_join_id);
            } else {
                subnet.connect_input(f_n - 1, right, 1);
            }
        }
        log::debug!("created sub network: {}", subnet.display());

        let subnet_id = net.push(Box::new(subnet));

        for (Node { f_n, .. }, id) in nodes_data.iter().zip(nodes.iter()) {
            net.connect(*id, 0, subnet_id, f_n - 1);
        }

        for n in 0..config.channels {
            net.connect_output(subnet_id, n, n);
        }

        let input_branch = dcblock() >> pass() ^ pass();
        let mut input_branch_id = net.push(Box::new(input_branch));
        net.connect_input(0, input_branch_id, 0);

        // let (anti, anti_id) = {

        //     let node = pass();
        //     let mut node_id = net.push(Box::new(node));
        //     for n in 0..config.channels {
        //         net.connect_input(n, node_id, 0);
        //         if n > 0 && n < config.channels - 1 {
        //             let node = join::<U2>();
        //             let next_node_id = net.push(Box::new(node));
        //             net.connect(node_id, 0, next_node_id, 1);
        //             node_id = next_node_id;
        //         }
        //     }

        //     (pass() ,node_id)
        // };

        // let input_branch = (dcblock() - anti) >> (pass() ^ pass());
        // let mut input_branch_id = net.push(Box::new(input_branch));
        // net.connect(anti_id, 0, input_branch_id, 1);

        // net.connect_input(config.channels, input_branch_id, 0);

        let mut rng_it = nodes.iter().peekable();

        while let Some(n) = rng_it.next() {
            if rng_it.peek().is_some() {
                net.connect(input_branch_id, 0, *n, 0);
                let input_branch = pass() >> pass() ^ pass();
                let next_input_branch_id = net.push(Box::new(input_branch));
                net.connect(input_branch_id, 1, next_input_branch_id, 0);
                input_branch_id = next_input_branch_id;
            } else {
                net.connect(input_branch_id, 0, *n, 0);
            }
        }

        net.set_sample_rate(config.sample_rate_hz);

        net.check();
        log::debug!("created network: {}", net.display());

        let mut net_be = BigBlockAdapter32::new(Box::new(net));

        net_be.allocate();

        Self {
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
