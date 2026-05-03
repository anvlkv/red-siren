use std::{ops::RemAssign, sync::Arc};

use common::instrument::Config as InstrumentConfig;
use common::tuner::Config as TunerConfig;
use fundsp::prelude::*;

pub mod excitor;
pub mod feedback_pass;
pub mod grid;
pub mod handle;
pub mod node;
pub mod ny_compressor;
pub mod values;

use values::FineTunedValues;

use crate::{
    excitor::{ExcitementSnapshot, SpectrumBuffer},
    handle::SystemHandle,
    node::mount_node_bands,
    rt::ExcitementSource,
};

/// Mounts the instrument system
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
) -> (Net, SystemHandle) {
    let net = Net::new(1, num_channels);

    let (xct, xct_handle) = excitor::create_excitor::<S>(xct_src, tuner_config, instrument_config);

    let _spectrum_buffer: SpectrumBuffer = xct_handle.analyzer.spectrum_buffer.clone();
    let xct_snapshot: ExcitementSnapshot<S> = xct.summary_snapshot();

    let xct = net.push(Box::new(xct));

    let ny_threshold = shared(0.5);
    let ny_wet_dry = shared(0.5);

    let _ny = ny_compressor::create_ny_compressor_thr_dry::<S>(&ny_threshold, &ny_wet_dry);

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
        grid::create_metro_tempo(bpm_tables, xct_snapshot) >> grid::create_rhythm_grid(),
    ));

    let node_bans = mount_node_bands(
        &mut net,
        instrument_config,
        values,
        xct,
        metro_grid,
        feedback_target,
    );

    let handle = SystemHandle {
        node_handles: Arc::new(node_bans),
        excitor_handle: Arc::new(xct_handle),
        input_ny_thr: Arc::new(ny_threshold),
        input_ny_wd: Arc::new(ny_wet_dry),
    };

    // fn mount_xct_parts(
    //     xct_src: ExcitementSource,
    //     instrument_config: InstrumentConfig,
    //     tuner_config: TunerConfig,
    //     net: &mut Net,
    // ) -> (NodeId, , ) {

    //     (net.push(Box::new(xct)), sf, ss)
    // }

    // let excitor_id = match sample_type {
    //     SampleType::F32 => subnet.push(Box::new(excitor::create_excitor::<f32>(
    //         xct_src,
    //         tuner_config.clone(),
    //         instrument_config.clone(),
    //     ))),
    //     SampleType::F64 => subnet.push(Box::new(excitor::create_excitor::<f64>(
    //         xct_src,
    //         tuner_config.clone(),
    //         instrument_config.clone(),
    //     ))),
    // };

    // let ny_id = match sample_type {
    //     SampleType::F32 => todo!(),
    //     SampleType::F64 => todo!(),
    // };

    // let metro_grid_id = match sample_type {
    //     SampleType::F32 => subnet.push(Box::new(grid::cr)),
    //     SampleType::F64 => subnet.push(Box::new(grid::cr)),
    // };

    (net, handle)
}

pub fn mount_input_system(
    _main_net: &mut Net,
    _config: &InstrumentConfig,
    _excitment_src: ExcitementSource,
) {
}
