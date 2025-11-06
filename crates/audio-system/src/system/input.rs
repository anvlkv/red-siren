mod new_york;

pub mod analyzer;
pub mod preamp;
pub mod random_excitor;

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
pub(crate) use random_excitor::RandomExcitor;

pub fn sensors_system(
    config: &Config,
    net: &mut Net,
    excitements: HashMap<NodeKey, Shared>,
    values: &FineTunedValues,
    spectrum_thb: &analyzer::SpectrumBuffer,
    tap_channel: usize,
) -> Vec<SensorHandles> {
    log::info!(
        "Creating sensors system with {} sensor configs and {} excitement controls",
        config.sensor_data.len(),
        excitements.len()
    );

    // Validate that sensor data and excitements have compatible NodeKeys
    let sensor_keys: std::collections::HashSet<NodeKey> =
        config.sensor_data.iter().map(|s| s.key).collect();
    let excitement_keys: std::collections::HashSet<NodeKey> = excitements.keys().copied().collect();

    let missing_excitements: Vec<NodeKey> =
        sensor_keys.difference(&excitement_keys).copied().collect();
    let orphaned_excitements: Vec<NodeKey> =
        excitement_keys.difference(&sensor_keys).copied().collect();

    if !missing_excitements.is_empty() {
        log::warn!(
            "Sensor configs without excitement controls: {:?}",
            missing_excitements
        );
    }
    if !orphaned_excitements.is_empty() {
        log::warn!(
            "Excitement controls without sensor configs: {:?}",
            orphaned_excitements
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

    // Create FFT analyzer with siren excitement
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
                    excitements,
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

/// Create a random excitement system that bypasses FFT analysis
/// Used when excitement source is Entropy/Random
pub fn randomized_system(_config: &Config, net: &mut Net, excitements: HashMap<NodeKey, Shared>) {
    log::info!(
        "Creating random sensors system with {} excitement controls",
        excitements.len()
    );

    // Create RandomExcitor node
    let random_excitor = RandomExcitor::new(excitements);

    // Add to network
    let id = net.push(Box::new(random_excitor));
    net.connect_input(0, id, 0);

    log::info!(
        "Random sensors system created successfully with excitor node id: {:?}",
        id
    );
}
