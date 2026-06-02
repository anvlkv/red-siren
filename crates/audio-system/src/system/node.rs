use std::marker::PhantomData;

use common::config::{self, NodeKey};
use fundsp::{prelude::*, typenum::Unsigned};
use u_num_it::u_num_it;

use crate::system::memo::memo;

pub struct Node<F: Real + 'static> {
    pub key: NodeKey,
    pub path_spread_coeff: Shared,
    pub mode_spacing_coeff: Shared,
    pub snoop: Snoop,
    paths_modal_net: Net,
    _sample_type: PhantomData<F>,
}

impl<F: Real + 'static> Node<F> {
    const SNOOP_SIZE: usize = 256;

    pub fn new(config: config::Node) -> Self {
        let mut paths_modal_net = Net::new(3, 1);
        let path_spread_coeff = Shared::new(0.0);
        let mode_spacing_coeff = Shared::new(0.0);

        let first_path_id = paths_modal_net.push(Self::resonant_path_modal(
            config,
            1,
            &path_spread_coeff,
            &mode_spacing_coeff,
        ));
        let second_path_id = paths_modal_net.push(Self::resonant_path_modal(
            config,
            2,
            &path_spread_coeff,
            &mode_spacing_coeff,
        ));
        let third_path_id = paths_modal_net.push(Self::feedback_path_modal(
            config,
            0,
            &path_spread_coeff,
            &mode_spacing_coeff,
        ));

        paths_modal_net.connect_input(0, first_path_id, 0);
        paths_modal_net.connect_input(1, second_path_id, 0);
        paths_modal_net.connect_input(2, third_path_id, 0);

        let (snoop, snoop_be) = snoop(Self::SNOOP_SIZE);

        let join_id = paths_modal_net.push(Box::new((pass() + pass() + pass()) >> snoop_be));

        paths_modal_net.connect(first_path_id, 0, join_id, 0);
        paths_modal_net.connect(second_path_id, 0, join_id, 1);
        paths_modal_net.connect(third_path_id, 0, join_id, 2);

        paths_modal_net.pipe_output(join_id);

        Self {
            key: config.key,
            paths_modal_net,
            path_spread_coeff,
            mode_spacing_coeff,
            snoop,
            _sample_type: PhantomData,
        }
    }

    pub fn backend(&mut self) -> NetBackend {
        self.paths_modal_net.backend()
    }

    fn feedback_path_modal(
        config: config::Node,
        path_index: usize,
        path_spread_coeff: &Shared,
        mode_spacing_coeff: &Shared,
    ) -> Box<dyn AudioUnit> {
        let indexed_resonator = |mode_index: usize| {
            (pass() | var(path_spread_coeff) | var(mode_spacing_coeff))
                >> (pass()
                    | memo(Map::new(
                        Self::node_mapper(config, path_index, mode_index as usize),
                        Routing::Split,
                    )))
                >> (resonator::<F>() * pass())
        };

        u_num_it!(
            [2, 5, 7, 11, 15, 20],
            match config.num_modes - 1 {
                U => {
                    type N = NumType;
                    Box::new(feedback2(
                        afollow(0.0, config.mode_decay_s / N::USIZE as f64) >> indexed_resonator(0),
                        noise()
                            * pipei::<N, _, _>(|mode_index| {
                                let loss = 0.995_f32.powi(mode_index as i32 + 1);
                                indexed_resonator(mode_index as usize + 1) >> mul(loss)
                            })
                            >> shape(Tanh(1.0)),
                    )) as Box<dyn AudioUnit>
                }
            }
        )
    }

    fn resonant_path_modal(
        config: config::Node,
        path_index: usize,
        path_spread_coeff: &Shared,
        mode_spacing_coeff: &Shared,
    ) -> Box<dyn AudioUnit> {
        u_num_it!(
            [3, 6, 8, 12, 16, 21],
            match config.num_modes {
                U => {
                    type N = NumType;
                    Box::new(
                        (pass() | var(path_spread_coeff) | var(mode_spacing_coeff))
                            >> multisplit::<U3, N>()
                            >> sumi::<N, _, _>(|mode_index| {
                                (pass()
                                    | memo(Map::new(
                                        Self::node_mapper(config, path_index, mode_index as usize),
                                        Routing::Split,
                                    )))
                                    >> (resonator::<F>() * map(|f: &Frame<f32, U1>| db_amp(f[0])))
                            }),
                    ) as Box<dyn AudioUnit>
                }
            }
        )
    }

    fn node_mapper(
        config: config::Node,
        path_index: usize,
        mode_index: usize,
    ) -> impl Fn(&Frame<f32, U2>) -> Frame<f32, U3> + Clone {
        move |f: &Frame<f32, U2>| {
            let path_spread_coeff = f[0] as f64;
            let mode_spacing_coeff = f[1] as f64;
            let inner_config = config::Node {
                mode_spacing_hz: config.mode_spacing_hz
                    + (config.mode_spacing_hz * mode_spacing_coeff),
                path_spacing_hz: config.path_spacing_hz
                    + (config.path_spacing_hz * path_spread_coeff),
                ..config
            };
            let frequency_hz = inner_config.path_mode_frequency_hz(path_index, mode_index);
            let q = inner_config.path_mode_q(path_index, mode_index);
            let gain = inner_config.mode_gain_db(mode_index);
            log::trace!(
                "Node mapper for path {}, mode {}: freq = {:.2} Hz, Q = {:.2}, gain = {:.2} dB",
                path_index,
                mode_index,
                frequency_hz,
                q,
                gain
            );
            Frame::<f32, U3>::from([frequency_hz as f32, q as f32, gain as f32])
        }
    }
}
