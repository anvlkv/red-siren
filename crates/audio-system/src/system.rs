use std::ops::RemAssign;

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
    rt::ExcitementSource,
};

/// Mounts the instrument system
pub fn mount_system<
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
    _values: &FineTunedValues,
) -> Net {
    let net = Net::new(1, num_channels);

    let (xct, xct_handle) = excitor::create_excitor::<S>(xct_src, tuner_config, instrument_config);

    let _spectrum_buffer: SpectrumBuffer = xct_handle.analyzer.spectrum_buffer.clone();
    let _xct_snapshot: ExcitementSnapshot<S> = xct.summary_snapshot();

    let ny_threshold = shared(0.5);
    let ny_wet_dry = shared(0.5);

    let _ny = ny_compressor::create_ny_compressor_thr_dry::<S>(&ny_threshold, &ny_wet_dry);

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

    net
}

pub fn mount_input_system(
    _main_net: &mut Net,
    _config: &InstrumentConfig,
    _excitment_src: ExcitementSource,
) {
}
