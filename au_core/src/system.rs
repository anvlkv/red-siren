pub mod backend;
mod node;

use std::collections::{HashMap, HashSet};

use crux_core::{
    capability::{CapabilityContext, Operation},
    Capability,
};
use fundsp::hacker32::*;
use hecs::Entity;
use serde::{Deserialize, Serialize};
use spectrum_analyzer::{
    samples_fft_to_spectrum, scaling::divide_by_N_sqrt, windows::hann_window, FrequencyLimit,
};

use ::shared::{FFTData, SnoopsData};
use node::Node;

pub struct System {
    pub output_net: Net32,
    pub input_net: Net32,
    pub nodes: HashMap<Entity, Node>,
    pub sample_rate: u32,
    pressed: HashSet<Entity>,
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
            sample_rate,
            nodes: Default::default(),
            pressed: Default::default(),
        }
    }

    pub fn replace_nodes(&mut self, nodes: Vec<Node>) -> Vec<(Snoop<f32>, Entity)> {
        let snoop_pairs = nodes.iter().map(|_| snoop(SNOOP_SIZE)).collect::<Vec<_>>();
        let (snoops, snoop_bes): (Vec<_>, Vec<_>) = snoop_pairs.into_iter().unzip();

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

        let (output_node, input_node): (Box<dyn AudioUnit32>, Box<dyn AudioUnit32>) = u_num_it!(
            1..=100,
            match nodes.len() {
                U => (
                    Box::new(bus::<U, _, _>(render_node)),
                    Box::new(bus::<U, _, _>(preamp_node)),
                ),
            }
        );

        _ = self.output_net.replace(self.output_bus_id, output_node);
        _ = self.input_net.replace(self.input_bus_id, input_node);
        self.input_net.commit();
        self.output_net.commit();

        let out_snoops = snoops
            .into_iter()
            .zip(nodes.iter().map(|n| n.button.clone()))
            .collect();

        self.nodes.clear();
        self.nodes.extend(nodes.into_iter().map(|n| (n.button, n)));
        self.pressed.clear();

        out_snoops
    }

    pub fn press(&mut self, entity: Entity, val: bool) {
        if val {
            _ = self.pressed.insert(entity);
        } else {
            _ = self.pressed.remove(&entity);
        }
    }

    pub fn move_f(&self, entity: Entity, val: f32) {
        if self.pressed.contains(&entity) {
            let node = self.nodes.get(&entity).expect("node");
            let base = node.f_base.value();
            node.f_emit.0.set_value(base + base * val);
            node.f_emit.1.set_value(base * 2.0 + base * (1.0 - val))
        }
    }

    pub fn control_node(&self, entity: &Entity, value: f32) {
        if let Some(node) = self.nodes.get(entity) {
            node.control.set_value(value);
        } else {
            log::error!("no node for entity");
        }
    }

    pub fn process_input_data(
        samples: &[f32],
        nodes: &HashMap<Entity, Node>,
        sample_rate: u32,
    ) -> FFTData {
        let hann_window = hann_window(samples);

        let spectrum_hann_window = samples_fft_to_spectrum(
            &hann_window,
            sample_rate,
            FrequencyLimit::Range(MIN_F, MAX_F),
            Some(&divide_by_N_sqrt),
        )
        .unwrap();

        let data = spectrum_hann_window
            .data()
            .iter()
            .map(|(f, v)| (f.val(), v.val()))
            .collect::<Vec<_>>();

        for (_, node) in nodes.iter() {
            let (min_fq, max_fq) = (node.f_sense.0 .0.value(), node.f_sense.0 .1.value());
            let (min_value, max_value) = (node.f_sense.1 .0.value(), node.f_sense.1 .1.value());
            let n_breadth = data
                .iter()
                .filter(|(freq, _)| *freq >= min_fq && *freq <= max_fq)
                .count();

            let activation = data.iter().fold(0.0, |acc, (freq, value)| {
                if *freq >= min_fq && *freq <= max_fq && *value >= min_value && *value <= max_value
                {
                    acc + (1.0 / n_breadth as f32)
                } else {
                    acc
                }
            });

            if activation > 0.0 {
                log::info!("activated node {} by {}", node.f_base.value(), activation)
            } else if node.control.value() > 0.0 {
                log::info!("deactivated node {}", node.f_base.value())
            }

            node.control.set_value(activation)
        }

        data
    }

    pub fn backends(&mut self) -> (BigBlockAdapter32, BlockRateAdapter32) {
        let (input, output) = (self.input_net.backend(), self.output_net.backend());
        (
            BigBlockAdapter32::new(Box::new(input)),
            BlockRateAdapter32::new(Box::new(output)),
        )
    }
}

#[derive(Serialize, Deserialize)]
pub enum SystemOp {
    #[serde(skip)]
    Backends(BigBlockAdapter32, BlockRateAdapter32, uuid::Uuid),
}

impl std::fmt::Debug for SystemOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SystemOp::Backends(_, _, id) => write!(f, "SystemOp::Backends {}", id),
        }
    }
}

impl PartialEq for SystemOp {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (SystemOp::Backends(_, _, id1), SystemOp::Backends(_, _, id2)) => id1 == id2,
        }
    }
}

impl Operation for SystemOp {
    type Output = bool;
}

pub struct SystemCapability<Ev> {
    context: CapabilityContext<SystemOp, Ev>,
}

impl<Ev> Capability<Ev> for SystemCapability<Ev> {
    type Operation = SystemOp;

    type MappedSelf<MappedEv> = SystemCapability<MappedEv>;

    fn map_event<F, NewEv>(&self, f: F) -> Self::MappedSelf<NewEv>
    where
        F: Fn(NewEv) -> Ev + Send + Sync + Copy + 'static,
        Ev: 'static,
        NewEv: 'static + Send,
    {
        SystemCapability::new(self.context.map_event(f))
    }
}

impl<Ev> SystemCapability<Ev>
where
    Ev: Send + 'static,
{
    pub fn new(context: CapabilityContext<SystemOp, Ev>) -> Self {
        Self { context }
    }

    pub fn send_be<F>(&self, sys: &mut System, notify: F)
    where
        F: Fn(bool) -> Ev + Send + 'static,
    {
        self.context.spawn({
            let (input, output) = sys.backends();
            let context = self.context.clone();
            async move {
                let done = context
                    .request_from_shell(SystemOp::Backends(input, output, uuid::Uuid::new_v4()))
                    .await;
                context.update_app(notify(done))
            }
        })
    }
}
