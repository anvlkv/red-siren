use std::{ops::RemAssign, sync::Arc};

use common::instrument::Config as InstrumentConfig;
use common::tuner::Config as TunerConfig;
use fundsp::prelude::*;

pub mod excitor;
pub mod feedback_pass;
pub mod grid;
pub mod handle;
pub mod mixer;
pub mod node;
pub mod ny_compressor;
pub mod values;

use values::FineTunedValues;

use crate::{
    excitor::ExcitementSnapshot, handle::SystemHandle, node::mount_node_bands, rt::ExcitementSource,
};

/// Creates the instrument system and handles controlling it
pub fn create_system<
    S: Float
        + Real
        + ordered_float::Float
        + ordered_float::FloatCore
        + RemAssign
        + PartialOrd
        + 'static,
>(
    num_channels: usize,
    xct_src: ExcitementSource,
    instrument_config: &InstrumentConfig,
    tuner_config: &TunerConfig,
    values: &FineTunedValues,
    seed: Option<u64>,
) -> (Net, SystemHandle) {
    let (subnet, handle) =
        create_instrument_system::<S>(xct_src, instrument_config, tuner_config, values, seed);

    let mut net = Net::new(subnet.inputs(), num_channels);

    let subnet_id = net.push(Box::new(subnet));

    net.pipe_input(subnet_id);

    let mixer_id = u_num_it::u_num_it!(
        1..=8,
        match num_channels {
            U => {
                net.push(Box::new(mixer::create_mixer::<S, U2, NumType>()))
            }
        }
    );

    net.pipe_all(subnet_id, mixer_id);

    net.pipe_output(mixer_id);

    net.check();

    (net, handle)
}

fn create_instrument_system<
    S: Float
        + Real
        + ordered_float::Float
        + ordered_float::FloatCore
        + RemAssign
        + PartialOrd
        + 'static,
>(
    xct_src: ExcitementSource,
    instrument_config: &InstrumentConfig,
    tuner_config: &TunerConfig,
    values: &FineTunedValues,
    seed: Option<u64>,
) -> (Net, SystemHandle) {
    let mut net = Net::new(1, 2);

    let (xct, xct_handle) = match seed {
        None => excitor::create_excitor::<S>(xct_src, tuner_config, instrument_config),
        Some(seed) => {
            excitor::create_seeded_excitor(xct_src, tuner_config, instrument_config, seed)
        }
    };

    let xct_snapshot: ExcitementSnapshot<S> = xct.summary_snapshot();

    let xct = net.push(Box::new(xct));

    let ny_threshold = shared(0.5);
    let ny_wet_dry = shared(0.5);

    let ny = net.push(Box::new(ny_compressor::create_ny_compressor_thr_dry::<S>(
        &ny_threshold,
        &ny_wet_dry,
    )));

    let num_nodes = instrument_config.num_nodes_total();
    let bpm_tables = instrument_config.bpm_tables();

    let feedback_target = u_num_it::u_num_it!(
        1..128,
        match num_nodes {
            U => {
                net.push(Box::new(multipass::<NumType>()))
            }
        }
    );

    let metro_grid = net.push(Box::new(
        grid::create_metro_tempo::<S>(bpm_tables, xct_snapshot) >> grid::create_rhythm_grid::<S>(),
    ));

    let node_bands = mount_node_bands::<S>(
        &mut net,
        instrument_config,
        values,
        xct,
        metro_grid,
        feedback_target,
    );

    let handle = SystemHandle {
        node_handles: Arc::new(node_bands),
        excitor_handle: Arc::new(xct_handle),
        input_ny_thr: Arc::new(ny_threshold),
        input_ny_wd: Arc::new(ny_wet_dry),
    };

    net.connect_input(0, ny, 0);

    match xct_src {
        ExcitementSource::Entropy => {}
        ExcitementSource::Mic => {
            net.connect(ny, 0, xct, 0);
        }
        ExcitementSource::Manual => {
            handle
                .excitor_handle
                .controls
                .values()
                .enumerate()
                .for_each(|(i, c)| {
                    let id = net.push(Box::new(c.control_node()));
                    net.connect(id, 0, xct, i);
                });
        }
    }

    net.pipe_all(feedback_target, xct);

    net.check();

    (net, handle)
}
