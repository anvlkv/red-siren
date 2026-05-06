use std::sync::Arc;

use common::{instrument::NodeConfig, NodeKey};
use fundsp::prelude::*;
use insta_fun::prelude::*;
use num_complex::Complex;
use ordered_float::OrderedFloat;
use parking_lot::RwLock;

use crate::excitor::{ExcitementData, ExcitementSnapshot};

use super::{
    adsr_shape_for_node, create_metro_tempo, create_rhythm_grid, create_rhythm_grid_envelope,
};

fn integration_config(num_samples: usize) -> SnapshotConfig {
    SnapshotConfigBuilder::default()
        .sample_rate(100.0)
        .num_samples(num_samples)
        .chart_layout(Layout::CombinedPerChannelType)
        .build()
        .unwrap()
}

fn x(re: f32, im: f32) -> ExcitementData<f32> {
    Complex::new(OrderedFloat(re), OrderedFloat(im))
}

fn bpm_tables() -> Vec<Vec<usize>> {
    vec![vec![60, 180]]
}

fn calm_weights() -> Vec<ExcitementData<f32>> {
    vec![x(1.0, 0.0), x(1.0, 0.0)]
}

fn agitated_weights() -> Vec<ExcitementData<f32>> {
    vec![x(0.25, 0.0), x(1.75, 1.0)]
}

fn pulse_schedule_input(start: f32, duration: f32) -> InputSource {
    InputSource::Generator(Box::new(move |i, ch| {
        if i == 0 {
            if ch == 0 {
                start
            } else {
                duration
            }
        } else {
            f32::NAN
        }
    }))
}

fn weighted_snapshot(weights: Vec<ExcitementData<f32>>) -> ExcitementSnapshot<f32> {
    Arc::new(RwLock::new(weights))
}

#[test]
fn grid_integration_metro_rhythm_envelope_calm_weights() {
    let metro = create_metro_tempo::<f32>(bpm_tables(), weighted_snapshot(calm_weights()));

    let env = create_rhythm_grid_envelope::<f32, _, _>(
        dc(1.0f32),
        adsr_shape_for_node::<f32>(&NodeConfig {
            key: NodeKey(0, 0),
            frequency: 440.0,
            phase: 0.0,
            cents: 100.0,
            l_mm: 170.0,
            w_kg: 0.1,
            v_cm3: 1.0,
        }),
    );

    let grid_pipeline = ((metro >> create_rhythm_grid::<f32>()) | pass() | pass()) >> env;

    assert_audio_unit_snapshot!(
        "grid_integration_metro_rhythm_envelope_calm_weights",
        grid_pipeline,
        pulse_schedule_input(0.0, 0.75),
        integration_config(360)
    );
}

#[test]
fn grid_integration_metro_rhythm_envelope_agitated_weights() {
    let metro = create_metro_tempo::<f32>(bpm_tables(), weighted_snapshot(agitated_weights()));

    let env = create_rhythm_grid_envelope::<f32, _, _>(
        dc(1.0f32),
        adsr_shape_for_node::<f32>(&NodeConfig {
            key: NodeKey(0, 1),
            frequency: 440.0,
            phase: 0.0,
            cents: 100.0,
            l_mm: 170.0,
            w_kg: 0.1,
            v_cm3: 1.0,
        }),
    );

    let grid_pipeline = ((metro >> create_rhythm_grid::<f32>()) | pass() | pass()) >> env;

    assert_audio_unit_snapshot!(
        "grid_integration_metro_rhythm_envelope_agitated_weights",
        grid_pipeline,
        pulse_schedule_input(0.0, 0.75),
        integration_config(360)
    );
}

