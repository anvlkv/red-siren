use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use fundsp::hacker32::*;
use hecs::Entity;
use parking_lot::Mutex;

use crate::node::*;

pub struct System {
    pub output_net: Net32,
    pub input_net: Net32,
    pub snoops: Arc<Mutex<Vec<(Snoop<f32>, Entity)>>>,
    pub nodes: Arc<Mutex<HashMap<Entity, Node>>>,
    pressed: Arc<Mutex<HashSet<Entity>>>,
    output_bus_id: NodeId,
    input_bus_id: NodeId,
    volume_l: Shared<f32>,
    volume_r: Shared<f32>,
}

pub const MOMENT: f32 = 1.0 / 75.0;
pub const MIN_F: f32 = 60.0;
pub const MAX_F: f32 = 18_000.0;
pub const SNOOP_SIZE: usize = 512;

impl System {
    pub fn new(sample_rate: u32) -> Self {
        let volume_l = shared(1.0);
        let volume_r = shared(1.0);

        let mut output_net = Net32::new(0, 2);
        let output_bus_id = output_net.push(Box::new(zero() | zero()));
        output_net.pipe_output(output_bus_id);
        output_net.set_sample_rate(sample_rate as f64);
        output_net.allocate();

        let mut input_net = Net32::new(1, 1);
        let input_bus_id = input_net.push(Box::new(pass()));
        input_net.pipe_input(input_bus_id);
        input_net.pipe_output(input_bus_id);
        input_net.set_sample_rate(sample_rate as f64);
        input_net.allocate();
        input_net.set_sample_rate(sample_rate as f64);

        Self {
            output_net,
            input_net,
            output_bus_id,
            input_bus_id,
            volume_l,
            volume_r,
            snoops: Default::default(),
            nodes: Default::default(),
            pressed: Default::default(),
        }
    }

    pub fn replace_nodes(&mut self, nodes: Vec<Node>) {
        let snoop_pairs = nodes.iter().map(|_| snoop(SNOOP_SIZE)).collect::<Vec<_>>();
        let (snoops, snoop_bes): (Vec<_>, Vec<_>) = snoop_pairs.into_iter().unzip();
        {
            let mut s_mtx = self.snoops.lock();
            s_mtx.clear();
            s_mtx.extend(snoops.into_iter().zip(nodes.iter().map(|n| n.button)));
        }

        let size = nodes.len();

        let render_node = |i: i64| {
            let (node_data, snoop) = i
                .try_into()
                .ok()
                .map(|i: usize| nodes.iter().zip(snoop_bes.iter()).nth(i))
                .flatten()
                .expect("node data for {i}");

            let i_amp = (size - i as usize) as f32 * 1.75;

            let control = (var(&node_data.control) * i_amp)
                >> adsr_live(MOMENT, MOMENT, MOMENT * (i + 1) as f32, MOMENT)
                >> follow(MOMENT * i as f32);

            let pluck_ring = pluck((75 * (i + 1) * 2) as f32, MOMENT, 0.0015);

            let osc = sine() * resonator();

            (var(&node_data.f_emit.0)
                | control)
                >> (pass()
                    | pluck_ring
                    | var(&node_data.f_emit.1) * 2.0 // hold sampling
                    | var(&node_data.f_base) * constant((i + 1) as f32) // resonator cutoff 
                    | (var(&node_data.f_emit.1) - var(&node_data.f_emit.0))) // resonator band width
                >> (pass()
                    | pinkpass()
                    | hold(0.75 / (i + 1) as f32)
                    | clip_to(0.0, 22_000.0))
                >> osc // osc
                >> declick_s(0.75)
                >> (pass() | var(&node_data.f_emit.1) | constant(0.75) | constant(1.75))
                >> bell()
                >> chorus(size as i64, MOMENT, MOMENT, 0.75)
                >> snoop.clone()
                >> (pass() | var(&node_data.pan))
                >> panner()
                >> (pass() * var(&self.volume_l) | pass() * var(&self.volume_r))
        };

        let preamp_node = |i: i64| {
            let node_data = i
                .try_into()
                .ok()
                .map(|i: usize| nodes.get(i))
                .flatten()
                .expect("node data for {i}");

            let p_hz = var(&node_data.f_sense.0 .0)
                + ((var(&node_data.f_sense.0 .1) - var(&node_data.f_sense.0 .0))
                    >> map(|i: &Frame<f32, U1>| i[0] / 2.0));
            let p_q = var(&node_data.f_sense.1 .1);
            let f_mul = var(&node_data.f_sense.0 .1) - var(&node_data.f_sense.0 .0);

            ((pass()) | p_hz | p_q) >> peak() >> pass() * f_mul
        };

        let (output_node, input_node): (Box<dyn AudioUnit32>, Box<dyn AudioUnit32>) =
            match nodes.len() {
                1 => (
                    Box::new(bus::<U1, _, _>(render_node)),
                    Box::new(bus::<U1, _, _>(preamp_node)),
                ),
                2 => (
                    Box::new(bus::<U2, _, _>(render_node)),
                    Box::new(bus::<U2, _, _>(preamp_node)),
                ),
                3 => (
                    Box::new(bus::<U3, _, _>(render_node)),
                    Box::new(bus::<U3, _, _>(preamp_node)),
                ),
                4 => (
                    Box::new(bus::<U4, _, _>(render_node)),
                    Box::new(bus::<U4, _, _>(preamp_node)),
                ),
                5 => (
                    Box::new(bus::<U5, _, _>(render_node)),
                    Box::new(bus::<U5, _, _>(preamp_node)),
                ),
                6 => (
                    Box::new(bus::<U6, _, _>(render_node)),
                    Box::new(bus::<U6, _, _>(preamp_node)),
                ),
                7 => (
                    Box::new(bus::<U7, _, _>(render_node)),
                    Box::new(bus::<U7, _, _>(preamp_node)),
                ),
                8 => (
                    Box::new(bus::<U8, _, _>(render_node)),
                    Box::new(bus::<U8, _, _>(preamp_node)),
                ),
                9 => (
                    Box::new(bus::<U9, _, _>(render_node)),
                    Box::new(bus::<U9, _, _>(preamp_node)),
                ),
                10 => (
                    Box::new(bus::<U10, _, _>(render_node)),
                    Box::new(bus::<U10, _, _>(preamp_node)),
                ),
                _ => panic!("empty system"),
            };

        _ = self.output_net.replace(self.output_bus_id, output_node);
        _ = self.input_net.replace(self.input_bus_id, input_node);
        self.input_net.commit();
        self.output_net.commit();

        {
            let mut n_mtx = self.nodes.lock();

            n_mtx.clear();
            n_mtx.extend(nodes.into_iter().map(|n| (n.button, n)));
        }
        {
            let mut p_mtx = self.pressed.lock();

            p_mtx.clear();
        }
    }

    pub fn press(&self, entity: Entity, val: bool) {
        let mut pressed = self.pressed.lock();

        if val {
            _ = pressed.insert(entity);
        } else {
            _ = pressed.remove(&entity);
        }
    }

    pub fn move_f(&self, entity: Entity, val: f32) {
        let pressed = self.pressed.lock();

        if pressed.contains(&entity) {
            let nodes = self.nodes.lock();

            let node = nodes.get(&entity).expect("node");
            let base = node.f_base.value();
            node.f_emit.0.set_value(base + base * val);
            node.f_emit.1.set_value(base * 2.0 + base * (1.0 - val))
        }
    }
}
