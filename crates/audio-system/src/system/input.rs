mod new_york;

pub mod analyzer;
pub mod preamp;
pub mod random_activator;

use std::collections::HashMap;

use common::tuner::Config;
use common::NodeKey;
#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;
use u_num_it::u_num_it;

use crate::system::values::FineTunedValues;

use super::SensorHandles;

pub(crate) use analyzer::FFTAnalyzer;
pub(crate) use random_activator::RandomActivator;

pub fn sensors_system(
    config: &Config,
    net: &mut Net,
    activations: HashMap<NodeKey, Shared>,
    values: &FineTunedValues,
    spectrum_thb: &analyzer::SpectrumBuffer,
    tap_channel: usize,
) -> Vec<SensorHandles> {
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

    let sensor_handles = create_sensor_handles(config);

    let sensor_shared = sensor_handles
        .iter()
        .flat_map(|h| {
            [
                &h.min_frequency,
                &h.max_frequency,
                &h.min_magnitude,
                &h.max_magnitude,
            ]
        })
        .collect::<Vec<&Shared>>();

    let sensor_inputs = sensor_shared.len();

    // Create FFT analyzer with siren activation
    let analyzer = u_num_it!(
        [
            0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 60, 64, 68, 72, 76, 80, 84,
            88, 92, 96, 100, 104, 108, 112, 116, 120, 124, 128, 132, 136, 140, 144, 148, 152, 156,
            160, 164, 168, 172, 176, 180, 184, 188, 192, 196, 200, 204, 208, 212, 216, 220, 224,
            228, 232, 236, 240, 244, 248, 252, 256, 260, 264, 268, 272, 276, 280, 284
        ], // Multiples of 4 up to 284 (71 sensors * 4 params)
        match sensor_inputs {
            U => {
                type SensorInputs = NumType;

                let stacks = preamp::create_sensors_preamp(values)
                    | stacki::<SensorInputs, _, _>(|i| var(sensor_shared[i as usize]));

                FFTAnalyzer::new(
                    Box::new(stacks),
                    config.clone(),
                    activations,
                    spectrum_thb.clone(),
                )
            }
            _ => {
                panic!("unexpected number of sesnsors")
            }
        }
    );

    // Add analyzer to the network
    let id = net.push(Box::new(analyzer));
    net.connect_input(0, id, 0);
    net.connect_output(id, 0, tap_channel);

    log::info!(
        "Sensors system created successfully with analyzer node id: {:?}",
        id
    );

    sensor_handles
}

/// Create shared values for sensor parameters
fn create_sensor_handles(config: &Config) -> Vec<SensorHandles> {
    let mut handles = Vec::with_capacity(config.sensor_data.len());

    for sensor in &config.sensor_data {
        handles.push(SensorHandles {
            key: sensor.key,
            min_frequency: shared(sensor.min_frequency),
            max_frequency: shared(sensor.max_frequency),
            min_magnitude: shared(sensor.min_magnitude),
            max_magnitude: shared(sensor.max_magnitude),
        });
    }

    handles
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
