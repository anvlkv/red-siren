use std::sync::Arc;
use std::{collections::HashMap, f32};

use common::instrument::layout::layout_test_cases;
use common::tuner::Config;
use fundsp::net::Net;
use fundsp::prelude::*;
use fundsp::thingbuf::ThingBuf;
use insta_fun::prelude::*;

use crate::rt::ExcitementSource;
#[cfg(feature = "editor")]
use crate::system::values::{FineTunedSharedValues, FineTunedValues};
use crate::system::{create_input_system, mount_output_system};

#[test]
fn instrument_with_rand_src() {
    fastrand::seed(42);

    for layout in layout_test_cases() {
        let instrument_config =
            common::instrument::Config::try_from(layout).expect("valid instrument config");
        let tuner_layout: common::tuner::layout::Layout = layout.into();
        let tuner_config = common::tuner::Config::new(
            tuner_layout,
            44_100.0,
            crate::input::analyzer::FFT_WINDOW_SIZE,
            layout.registry(),
        );

        // Create a simple stereo output system network (no inputs)
        let mut net = Net::new(1, 2);
        net.set_sample_rate(44_100.0);

        #[cfg(feature = "editor")]
        let values = FineTunedValues::new(&FineTunedSharedValues::default());

        let node_handles = mount_output_system::<f32>(
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

        let cfg = Config::default();
        let ny_threshold = shared(cfg.ny_threshold);
        let ny_wet_ratio = shared(cfg.ny_wet_ratio);
        let min_freq = shared(f32::NEG_INFINITY);
        let max_freq = shared(f32::INFINITY);

        // Tuner default (Mic analyzer) hooked to a tap channel from the output
        let spectrum_thb = Arc::new(ThingBuf::new(10));
        let _sensor_handles = create_input_system::<f32>(
            &tuner_config,
            &mut net,
            excitements,
            ExcitementSource::Entropy,
            &spectrum_thb,
            None,
            (&min_freq, &max_freq),
            (&ny_threshold, &ny_wet_ratio),
            2, // tap channel
        );

        let case_title = format!("{}x{}_{:?}", layout.space.x, layout.space.y, layout.scale);

        let svg_config = SvgChartConfigBuilder::default()
            .show_grid(true)
            .chart_title(&case_title)
            .preserve_aspect_ratio(SvgPreserveAspectRatio::scale_to_fit())
            .build()
            .unwrap();

        let config = SnapshotConfigBuilder::default()
            .num_samples(2000)
            .warm_up(WarmUp::Seconds(1.0))
            .allow_abnormal_samples(true)
            .output_mode(svg_config)
            .build()
            .unwrap();

        assert_audio_unit_snapshot!(net.clone(), config);

        let config = SnapshotConfigBuilder::default()
            .num_samples(44100)
            .warm_up(WarmUp::Samples(8000))
            .output_mode(WavOutput::Wav32)
            .build()
            .unwrap();

        assert_audio_unit_snapshot!(case_title, net.clone(), InputSource::None, config);
    }
}

#[test]
fn instrument_with_mic_src() {
    fastrand::seed(42);

    let config = SnapshotConfigBuilder::default()
        .num_samples(2048)
        .warm_up(WarmUp::Samples(1000))
        .allow_abnormal_samples(true)
        .show_grid(true)
        .with_inputs(true)
        .build()
        .unwrap();

    let layout = layout_test_cases().next().unwrap();

    let instrument_config =
        common::instrument::Config::try_from(layout).expect("valid instrument config");
    let tuner_layout: common::tuner::layout::Layout = layout.into();
    let tuner_config = common::tuner::Config::new(
        tuner_layout,
        44_100.0,
        crate::input::analyzer::FFT_WINDOW_SIZE,
        layout.registry(),
    );

    // Create a stereo output + tuner (mic) analysis network.
    // We give the Net one input channel and three outputs (following earlier integration
    // layout where analyzer taps a channel index, and 2 are used by stereo out).
    let mut net = Net::new(1, 3);
    net.set_sample_rate(44_100.0);

    #[cfg(feature = "editor")]
    let values = FineTunedValues::new(&FineTunedSharedValues::default());

    // Output system
    let node_handles = mount_output_system::<f32>(
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

    let cfg = Config::default();
    let ny_threshold = shared(cfg.ny_threshold);
    let ny_wet_ratio = shared(cfg.ny_wet_ratio);
    let min_freq = shared(f32::NEG_INFINITY);
    let max_freq = shared(f32::INFINITY);

    // Tuner default (Mic analyzer) hooked to a tap channel from the output
    let spectrum_thb = Arc::new(ThingBuf::new(10));
    let _sensor_handles = create_input_system::<f32>(
        &tuner_config,
        &mut net,
        excitements,
        ExcitementSource::Mic,
        &spectrum_thb,
        None,
        (&min_freq, &max_freq),
        (&ny_threshold, &ny_wet_ratio),
        2, // tap channel,
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
