mod new_york;

pub mod analyzer;
pub mod preamp;
pub mod random_activator;

pub use analyzer::{FFTAnalyzer, FFT_WINDOW_SIZE};
pub use random_activator::RandomActivator;

use common::tuner::Config;
use common::NodeKey;
#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;
use preamp::create_sensors_preamp;
use std::collections::HashMap;

use crate::system::values::FineTunedValues;

pub fn sensors_system(
    config: &Config,
    net: &mut Net,
    activations: HashMap<NodeKey, Shared>,
    values: &FineTunedValues,
) {
    log::info!(
        "Creating sensors system with {} sensor configs and {} activation controls",
        config.sensor_data.len(),
        activations.len()
    );

    // Validate that sensor data and activations have compatible NodeKeys
    let sensor_keys: std::collections::HashSet<NodeKey> =
        config.sensor_data.iter().map(|s| s.key).collect();
    let activation_keys: std::collections::HashSet<NodeKey> = activations.keys().copied().collect();

    let missing_activations: Vec<NodeKey> =
        sensor_keys.difference(&activation_keys).copied().collect();
    let orphaned_activations: Vec<NodeKey> =
        activation_keys.difference(&sensor_keys).copied().collect();

    if !missing_activations.is_empty() {
        log::warn!(
            "Sensor configs without activation controls: {:?}",
            missing_activations
        );
    }
    if !orphaned_activations.is_empty() {
        log::warn!(
            "Activation controls without sensor configs: {:?}",
            orphaned_activations
        );
    }

    // Log NodeKey ranges for debugging
    if !sensor_keys.is_empty() {
        let max_group = sensor_keys.iter().map(|k| k.group()).max().unwrap_or(0);
        let max_key = sensor_keys.iter().map(|k| k.key()).max().unwrap_or(0);
        log::info!(
            "Sensor NodeKey ranges: groups 0-{}, keys 0-{}",
            max_group,
            max_key
        );
    }

    // Create preamp for input calibration
    let preamp = create_sensors_preamp(values);

    // Create FFT analyzer with siren activation
    let analyzer = FFTAnalyzer::new(
        Box::new(preamp),
        FFT_WINDOW_SIZE,
        config.clone(),
        activations,
    );

    // Add analyzer to the network
    let id = net.push(Box::new(analyzer));
    net.connect_input(0, id, 0);

    log::info!(
        "Sensors system created successfully with analyzer node id: {:?}",
        id
    );
}

/// Create a random activation system that bypasses FFT analysis
/// Used when activation source is Entropy/Random
pub fn randomized_system(_config: &Config, net: &mut Net, activations: HashMap<NodeKey, Shared>) {
    log::info!(
        "Creating random sensors system with {} activation controls",
        activations.len()
    );

    // Create RandomActivator node
    let random_activator = RandomActivator::new(activations);

    // Add to network
    let id = net.push(Box::new(random_activator));
    net.connect_input(0, id, 0);

    log::info!(
        "Random sensors system created successfully with activator node id: {:?}",
        id
    );
}
