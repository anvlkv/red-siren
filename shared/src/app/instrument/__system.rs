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
        let mut net = Net32::new(config.channels, config.channels);
        let entities: Vec<Entity> = System::spawn_nodes(world);
        let size = entities.len();
        let nodes_data = entities
            .iter()
            .map(|e| *world.get::<&Node>(*e).expect("node for entity"))
            .collect::<Vec<_>>();

        let mut amps = Vec::with_capacity(size);
        let mut tunes = Vec::with_capacity(size);
        let mut nodes = Vec::with_capacity(size);

        

        for n in 0..config.channels {
            let c = sine_hz(440.0);
            let id = net.push(Box::new(c));
            nodes.push(id);
            net.connect_output(id, 0, n);
        }
        
        



        net.set_sample_rate(config.sample_rate_hz);

        net.check();

        let mut net_be = net.backend();

        net_be.allocate();

        log::debug!("created network: {}", net.display());

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
