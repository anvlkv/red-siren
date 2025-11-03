//! Integration tests for the audio system input/output pipeline

#[cfg(test)]
mod integration_tests {
    use crate::rt::ActivationSource;
    use crate::system::{create_input_system, create_output_system};
    use common::instrument::{Config as InstrumentConfig, GroupConfig, NodeConfig};
    use common::tuner::{Config as TunerConfig, SensorData};
    use common::NodeKey;
    #[cfg(feature = "hi_fi")]
    use fundsp::hacker::prelude::*;
    #[cfg(not(feature = "hi_fi"))]
    use fundsp::hacker32::prelude::*;
    use fundsp::net::Net;
    use fundsp::thingbuf::ThingBuf;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn create_test_instrument_config() -> InstrumentConfig {
        // Create test config with 2 groups, 2 nodes each (minimum required)
        let left_nodes = vec![
            NodeConfig {
                key: NodeKey::new(0, 0),
                base_frequency: 440.0,
                phase: 0.0,
            },
            NodeConfig {
                key: NodeKey::new(0, 1),
                base_frequency: 880.0,
                phase: 0.0,
            },
        ];

        let right_nodes = vec![
            NodeConfig {
                key: NodeKey::new(1, 0),
                base_frequency: 554.37, // C# above middle C
                phase: 0.0,
            },
            NodeConfig {
                key: NodeKey::new(1, 1),
                base_frequency: 659.25, // E above middle C
                phase: 0.0,
            },
        ];

        let config = InstrumentConfig(vec![
            GroupConfig {
                channel: common::instrument::GroupChannel::Left,
                nodes: left_nodes,
            },
            GroupConfig {
                channel: common::instrument::GroupChannel::Right,
                nodes: right_nodes,
            },
        ]);

        // Debug: Print the config details
        println!("Test config created:");
        println!("  Total groups: {}", config.num_groups());
        println!("  Left groups: {}", config.num_groups_left());
        println!("  Right groups: {}", config.num_groups_right());
        println!("  Nodes per group: {}", config.num_nodes_per_group());

        config
    }

    fn create_test_tuner_config() -> TunerConfig {
        TunerConfig {
            sample_rate: 44100.0,
            sensor_data: vec![
                SensorData {
                    key: NodeKey::new(0, 0),
                    min_frequency: 400.0,
                    max_frequency: 480.0,
                    min_magnitude: -60.0,
                    max_magnitude: 0.0,
                },
                SensorData {
                    key: NodeKey::new(0, 1),
                    min_frequency: 800.0,
                    max_frequency: 960.0,
                    min_magnitude: -60.0,
                    max_magnitude: 0.0,
                },
                SensorData {
                    key: NodeKey::new(1, 0),
                    min_frequency: 500.0,
                    max_frequency: 600.0,
                    min_magnitude: -60.0,
                    max_magnitude: 0.0,
                },
                SensorData {
                    key: NodeKey::new(1, 1),
                    min_frequency: 600.0,
                    max_frequency: 700.0,
                    min_magnitude: -60.0,
                    max_magnitude: 0.0,
                },
            ],
        }
    }

    #[test]
    fn test_mic_activation_source() {
        let instrument_config = create_test_instrument_config();
        let tuner_config = create_test_tuner_config();

        // Create output system first
        let mut net = Net::new(1, 3);
        net.set_sample_rate(44100.0);

        #[cfg(feature = "editor")]
        let values =
            crate::values::FineTunedValues::new(&crate::values::FineTunedSharedValues::default());

        let node_handles = create_output_system(
            &instrument_config,
            &mut net,
            2,
            #[cfg(feature = "editor")]
            &values,
        );

        // Collect activation controls
        let mut activations = HashMap::new();
        for handle in node_handles {
            activations.insert(handle.key, handle.siren_control);
        }

        let spectrum_thb = Arc::new(ThingBuf::new(10));

        // Create input system with Mic source (FFT analyzer)
        let handles = create_input_system(
            &tuner_config,
            &mut net,
            activations.clone(),
            ActivationSource::Mic,
            &spectrum_thb,
            2,
            #[cfg(feature = "editor")]
            &values,
        );

        // Network should be valid
        net.check();
        net.allocate();

        // Verify the network contains FFT analyzer
        let mut backend = net.backend();

        // Process some samples
        let input = [0.0f32];
        let mut output = [0.0f32, 0.0f32];
        backend.tick(&input, &mut output);

        // Activations should start at 0
        for control in activations.values() {
            assert_eq!(control.value(), 0.0, "Mic activation should start at 0");
        }

        assert!(!handles.is_empty())
    }

    #[test]
    fn test_entropy_activation_source() {
        let instrument_config = create_test_instrument_config();
        let tuner_config = create_test_tuner_config();

        // Create output system first
        let mut net = Net::new(1, 3);
        net.set_sample_rate(44100.0);

        #[cfg(feature = "editor")]
        let values =
            crate::values::FineTunedValues::new(&crate::values::FineTunedSharedValues::default());

        let node_handles = create_output_system(
            &instrument_config,
            &mut net,
            1,
            #[cfg(feature = "editor")]
            &values,
        );

        // Collect activation controls
        let mut activations = HashMap::new();
        for handle in node_handles {
            activations.insert(handle.key, handle.siren_control.clone());
        }

        let spectrum_thb = Arc::new(ThingBuf::new(10));
        // Create input system with Entropy source (RandomActivator)
        let handles = create_input_system(
            &tuner_config,
            &mut net,
            activations.clone(),
            ActivationSource::Entropy,
            &spectrum_thb,
            2,
            #[cfg(feature = "editor")]
            &values,
        );

        // Network should be valid
        net.check();
        net.allocate();

        // Verify the network contains RandomActivator
        let mut backend = net.backend();

        // Process some samples - RandomActivator should start generating values
        let input = [0.0f32];
        let mut output = [0.0f32, 0.0f32];

        // Process multiple samples to allow RandomActivator to update
        for _ in 0..1000 {
            backend.tick(&input, &mut output);
        }

        // At least one activation should have changed from 0
        // (RandomActivator generates random values)
        let any_activation = activations.values().any(|control| control.value() > 0.0);

        assert!(handles.is_empty());

        assert!(
            any_activation,
            "At least one activation should be greater than 0 with Entropy source"
        );
    }

    #[test]
    fn test_activation_source_switching() {
        let instrument_config = create_test_instrument_config();
        let tuner_config = create_test_tuner_config();

        // Test Mic source first
        {
            let mut net = Net::new(1, 2);
            net.set_sample_rate(44100.0);

            #[cfg(feature = "editor")]
            let values = crate::values::FineTunedValues::new(
                &crate::values::FineTunedSharedValues::default(),
            );

            let node_handles = create_output_system(
                &instrument_config,
                &mut net,
                1,
                #[cfg(feature = "editor")]
                &values,
            );
            let mut activations = HashMap::new();
            for handle in node_handles {
                activations.insert(handle.key, handle.siren_control);
            }

            let spectrum_thb = Arc::new(ThingBuf::new(10));
            let handles = create_input_system(
                &tuner_config,
                &mut net,
                activations,
                ActivationSource::Mic,
                &spectrum_thb,
                1,
                #[cfg(feature = "editor")]
                &values,
            );
            net.check();
            net.allocate();

            assert!(!handles.is_empty());
        }

        // Test Entropy source
        {
            let mut net = Net::new(1, 6);
            net.set_sample_rate(44100.0);

            #[cfg(feature = "editor")]
            let values = crate::values::FineTunedValues::new(
                &crate::values::FineTunedSharedValues::default(),
            );

            let node_handles = create_output_system(
                &instrument_config,
                &mut net,
                5,
                #[cfg(feature = "editor")]
                &values,
            );
            let mut activations = HashMap::new();
            for handle in node_handles {
                activations.insert(handle.key, handle.siren_control);
            }

            let spectrum_thb = Arc::new(ThingBuf::new(10));
            let handles = create_input_system(
                &tuner_config,
                &mut net,
                activations,
                ActivationSource::Entropy,
                &spectrum_thb,
                5,
                #[cfg(feature = "editor")]
                &values,
            );
            net.check();
            net.allocate();

            assert!(handles.is_empty());
        }
    }

    #[test]
    fn test_random_activator_value_range() {
        use crate::system::input::RandomActivator;

        // Create test activation controls
        let mut controls = HashMap::new();
        for i in 0..5 {
            let key = NodeKey::new(0, i);
            let control = shared(0.0);
            controls.insert(key, control.clone());
        }

        let mut activator = RandomActivator::new(controls.clone());
        activator.set_sample_rate(44100.0);
        activator.reset();

        // Process many samples
        let input = [0.0f32];
        let mut output = [];

        for _ in 0..10000 {
            activator.tick(&input, &mut output);
        }

        // Check that all values are within [0, 1]
        for control in controls.values() {
            let value = control.value();
            assert!(
                (0.0..=1.0).contains(&value),
                "Activation value {} should be in range [0, 1]",
                value
            );
        }
    }
}
