use app_core::{
    instrument::{Config, Node},
    tuner::TuningValue,
};
use fundsp::hacker32::*;

pub const SAMPLE_RATE: f64 = 44100.0;
const SNOOP_SIZE: usize = 64;
const CHANNELS: usize = 2;
pub const MUL: f32 = 100000.0;

pub struct System {
    pub net_be: BigBlockAdapter32,
    pub size: usize,
    pub channels: usize,
    pub sample_rate: f64,
    pub nodes: Vec<NodeId>,
    pub node_snp: Vec<(Snoop<f32>, usize)>,
    pub b_centres: Vec<Shared<f32>>,
    pub b_qs: Vec<Shared<f32>>,
    pub n_fs: Vec<Shared<f32>>,
    pub out_snp: Snoop<f32>,
}

impl System {
    pub fn new(nodes_data: &[Node], config: &Config, tuning: &[TuningValue]) -> Self {
        let sample_rate = SAMPLE_RATE;
        let channels = Ord::min(config.groups, CHANNELS);
        let mut net = Net32::new(1, channels);
        net.set_sample_rate(sample_rate);

        let size = nodes_data.len();
        let mut nodes = vec![];
        let mut node_snp = vec![];
        let mut b_centres = vec![];
        let mut b_qs = vec![];
        let mut n_fs = vec![];

        let mut input_subnet = Net32::new(1, size);
        let mut output_subnet = Net32::new(size, channels);

        let input_pipe = declick_s(0.75);

        let input_pipe_id = match nodes_data.len() {
            2 => input_subnet.push(Box::new(input_pipe >> split::<U2>())),
            3 => input_subnet.push(Box::new(input_pipe >> split::<U3>())),
            4 => input_subnet.push(Box::new(input_pipe >> split::<U4>())),
            5 => input_subnet.push(Box::new(input_pipe >> split::<U5>())),
            6 => input_subnet.push(Box::new(input_pipe >> split::<U6>())),
            9 => input_subnet.push(Box::new(input_pipe >> split::<U9>())),
            10 => input_subnet.push(Box::new(input_pipe >> split::<U10>())),
            12 => input_subnet.push(Box::new(input_pipe >> split::<U12>())),
            n => todo!("support for {n} nodes"),
        };

        input_subnet.connect_input(0, input_pipe_id, 0);

        for (i, (node_data, tuning)) in nodes_data.iter().zip(tuning).enumerate() {
            assert_eq!(tuning.0, node_data.f_n, "pair f_n");

            let bp_f = shared(tuning.1);

            log::debug!(
                "f_n: {}, sensitize to: {}, freq: {}",
                node_data.f_n,
                tuning.1,
                node_data.freq.0
            );

            // todo: use hid input
            let bp_q = shared(1.0 / size as f32);
            let ch_mul = 1.0 + MUL - MUL * tuning.2;

            log::info!("amp channel input by {ch_mul}");
            let (n_snp, snp_an) = snoop(SNOOP_SIZE);
            node_snp.push((n_snp, node_data.f_n));
            let bp_n = mul(ch_mul)
                >> (pass() | var(&bp_f) | var(&bp_q))
                >> bandrez()
                >> pluck(node_data.freq.1, 0.75, 0.25);
                
            b_centres.push(bp_f);
            b_qs.push(bp_q);

            let bp_id = input_subnet.push(Box::new(bp_n));

            input_subnet.connect(input_pipe_id, i, bp_id, 0);
            input_subnet.connect_output(bp_id, 0, i);

            let n_f = shared(node_data.freq.0);
            let mut node = (var(&n_f) | pass()) >> (sine() * follow(0.075)) >> bell_hz(node_data.freq.1, 0.25, 1.75) >> snp_an;
            n_fs.push(n_f);

            log::debug!("created node: {}", node.display());

            let node_id = output_subnet.push(Box::new(node));

            output_subnet.connect_input(i, node_id, 0);

            nodes.push(node_id);
        }

        let (out_snp, an_snp) = snoop(SNOOP_SIZE);

        let output_pipe_id = match channels {
            1 => {
                let (r_f, d_f) = nodes_data
                    .last()
                    .map(|n| (n.freq.1 * 5.0, n.freq.1 - n.freq.0))
                    .unwrap();
                let r = resonator_hz(r_f, d_f) >> an_snp >> mul(10.0) >> pinkpass();

                match nodes_data.len() {
                    2 => output_subnet.push(Box::new(join::<U2>() >> r)),
                    3 => output_subnet.push(Box::new(join::<U3>() >> r)),
                    5 => output_subnet.push(Box::new(join::<U5>() >> r)),
                    n => todo!("support {n} nodes, 1 channel"),
                }
            }
            2 => {
                let (lr_f, ld_f) = nodes_data
                    .iter()
                    .filter(|n| n.pan < 0)
                    .last()
                    .map(|n| (n.freq.1 * 2.0, n.freq.1 - n.freq.0))
                    .unwrap();
                let (rr_f, rd_f) = nodes_data
                    .iter()
                    .filter(|n| n.pan > 0)
                    .last()
                    .map(|n| (n.freq.1 * 4.0, n.freq.1 - n.freq.0))
                    .unwrap();
                let r = (resonator_hz(lr_f, ld_f) | resonator_hz(rr_f, rd_f))
                    >> (split::<U2>() | split::<U2>())
                    >> (pass() | join::<U2>() | pass())
                    >> (pass() | an_snp | pass())
                    >> (pinkpass() | sink() | pinkpass());

                match nodes_data.len() {
                    4 => output_subnet.push(Box::new((join::<U2>() | join::<U2>()) >> r)),
                    6 => output_subnet.push(Box::new((join::<U3>() | join::<U3>()) >> r)),
                    9 => {
                        let right = nodes_data.iter().filter(|n| n.pan > 0).count();
                        let left = nodes_data.iter().filter(|n| n.pan < 0).count();
                        if right > left {
                            output_subnet.push(Box::new((join::<U3>() | join::<U6>()) >> r))
                        } else {
                            output_subnet.push(Box::new((join::<U6>() | join::<U3>()) >> r))
                        }
                    }
                    10 => output_subnet.push(Box::new((join::<U5>() | join::<U5>()) >> r)),
                    12 => output_subnet.push(Box::new((join::<U6>() | join::<U6>()) >> r)),
                    n => todo!("support {n} nodes, 2 channels"),
                }
            }
            n => todo!("support {n} channels"),
        };

        match channels {
            1 => {
                output_subnet.connect_output(output_pipe_id, 0, 0);
            }
            2 => {
                output_subnet.connect_output(output_pipe_id, 0, 0);
                output_subnet.connect_output(output_pipe_id, 1, 1);
            }
            n => todo!("support {n} channels"),
        }

        for (node_id, node_data) in nodes.iter().zip(nodes_data) {
            if node_data.pan > 0 && channels > 1 {
                output_subnet.connect(*node_id, 0, output_pipe_id, 1);
            } else {
                output_subnet.connect(*node_id, 0, output_pipe_id, 0);
            }
        }

        log::debug!("created input network: {}", input_subnet.display());
        log::debug!("created output network: {}", output_subnet.display());

        let in_id = net.push(Box::new(input_subnet));
        let out_id = net.push(Box::new(output_subnet));

        net.connect_input(0, in_id, 0);

        for ch in 0..channels {
            net.connect_output(out_id, ch, ch);
        }

        for i in 0..size {
            net.connect(in_id, i, out_id, i);
        }

        net.check();
        log::debug!("created network: {}", net.display());

        let mut net_be = BigBlockAdapter32::new(Box::new(net));

        net_be.allocate();

        Self {
            channels,
            sample_rate,
            net_be,
            size,
            b_centres,
            b_qs,
            n_fs,
            nodes,
            out_snp,
            node_snp
        }
    }
}
