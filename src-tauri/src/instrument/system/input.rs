mod analyzer;
mod preamp;

use std::collections::HashMap;

use analyzer::{FFTAnalyzer, FFT_WINDOW_SIZE};
use common::tuner::Config;
use common::NodeKey;
use fundsp::hacker32::prelude::*;
use preamp::create_sensors_preamp;

pub fn sensors_system(config: &Config, net: &mut Net, siren_controls: &HashMap<NodeKey, Shared>) {
    // Create preamp for input calibration
    let preamp = create_sensors_preamp();

    // Create FFT analyzer with siren activation
    let analyzer = FFTAnalyzer::new(
        Box::new(preamp),
        FFT_WINDOW_SIZE,
        config.clone(),
        siren_controls.clone(),
    );

    // Add analyzer to the network
    let id = net.push(Box::new(analyzer));
    net.connect_input(0, id, 0);
}
