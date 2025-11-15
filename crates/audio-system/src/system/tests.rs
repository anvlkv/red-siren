use crate::rt::ExcitementSource;
#[cfg(feature = "editor")]
use crate::system::values::{FineTunedSharedValues, FineTunedValues};
use crate::system::{create_input_system, create_output_system};
use common::instrument::layout::layout_test_cases;
#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;
use fundsp::net::Net;
use fundsp::thingbuf::ThingBuf;
use insta_fun::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

#[test]
fn instrument_with_rand_src() {
    let config = SnapshotConfigBuilder::default()
        .num_samples(2000)
        .allow_abnormal_samples(true)
        .show_grid(true)
        .build()
        .unwrap();

    let layout = layout_test_cases().next().unwrap();

    let instrument_config =
        common::instrument::Config::try_from(layout).expect("valid instrument config");
    let tuner_layout: common::tuner::layout::Layout = layout.into();
    let tuner_config = common::tuner::Config::new(tuner_layout, 44_100.0, layout.registry());

    // Create a simple stereo output system network (no inputs)
    let mut net = Net::new(1, 2);
    net.set_sample_rate(44_100.0);

    #[cfg(feature = "editor")]
    let values = FineTunedValues::new(&FineTunedSharedValues::default());

    let node_handles = create_output_system(
        &instrument_config,
        &mut net,
        2, // stereo
        #[cfg(feature = "editor")]
        &values,
    );

    // Collect siren excitement controls keyed by node
    let mut excitements = HashMap::new();
    for h in node_handles {
        excitements.insert(h.key, h.siren_control);
    }

    // Tuner default (Mic analyzer) hooked to a tap channel from the output
    let spectrum_thb = Arc::new(ThingBuf::new(10));
    let _sensor_handles = create_input_system(
        &tuner_config,
        &mut net,
        excitements,
        ExcitementSource::Entropy,
        &spectrum_thb,
        2, // tap channel
        #[cfg(feature = "editor")]
        &values,
    );

    assert_audio_unit_snapshot!(net, config);
}

#[test]
fn instrument_with_mic_src() {
    let config = SnapshotConfigBuilder::default()
        .num_samples(2048)
        .allow_abnormal_samples(true)
        .show_grid(true)
        .with_inputs(true)
        .build()
        .unwrap();

    let layout = layout_test_cases().next().unwrap();

    let instrument_config =
        common::instrument::Config::try_from(layout).expect("valid instrument config");
    let tuner_layout: common::tuner::layout::Layout = layout.into();
    let tuner_config = common::tuner::Config::new(tuner_layout, 44_100.0, layout.registry());

    // Create a stereo output + tuner (mic) analysis network.
    // We give the Net one input channel and three outputs (following earlier integration
    // layout where analyzer taps a channel index, and 2 are used by stereo out).
    let mut net = Net::new(1, 3);
    net.set_sample_rate(44_100.0);

    #[cfg(feature = "editor")]
    let values = FineTunedValues::new(&FineTunedSharedValues::default());

    // Output system
    let node_handles = create_output_system(
        &instrument_config,
        &mut net,
        2, // stereo
        #[cfg(feature = "editor")]
        &values,
    );

    // Collect siren excitement controls keyed by node
    let mut excitements = HashMap::new();
    for h in node_handles {
        excitements.insert(h.key, h.siren_control);
    }

    // Tuner default (Mic analyzer) hooked to a tap channel from the output
    let spectrum_thb = Arc::new(ThingBuf::new(10));
    let _sensor_handles = create_input_system(
        &tuner_config,
        &mut net,
        excitements,
        ExcitementSource::Mic,
        &spectrum_thb,
        2, // tap channel
        #[cfg(feature = "editor")]
        &values,
    );

    let input = {
        use fastrand::Rng;

        let seed = 42;

        let mut rng = Rng::with_seed(seed);

        InputSource::VecByChannel(vec![Vec::from_iter((0..2048).map(|_| {
            if rng.bool() {
                -rng.f32()
            } else {
                rng.f32()
            }
        }))])
    };

    assert_audio_unit_snapshot!("emulated mic noise", net, input, config);
}
