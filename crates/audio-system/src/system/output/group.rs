use std::{cell::RefCell, collections::HashMap, f32, f64};

use common::{instrument::GroupConfig, NodeKey};
use fundsp::{
    prelude::*,
    typenum::{UInt, UTerm, B1},
};

use super::InnerHandles;

use crate::{
    output::{
        metro::{metro_busi, MetroBusType},
        node::NodeType,
    },
    system::values::{FineTunedValue, FineTunedValues},
    util::DbLin,
};
use crate::{
    output::{
        node::create_node,
        throw_catch::{ThrowCatchCatch, ThrowCatchThrow},
    },
    util::S,
};

type ShelfType = Pipe<
    Stack<Stack<Stack<Pass, Constant<UInt<UTerm, B1>>>, FineTunedValue>, DbLin>,
    Svf<S, HighshelfMode<S>>,
>;

type BandSum<K> = Pipe<
    Constant<U1>,
    Chain<
        K,
        Binop<
            FrameAdd<U1>,
            Pass,
            Pipe<
                Binop<FrameSub<U1>, Constant<U1>, Binop<FrameMul<U1>, Var, Constant<U1>>>,
                Binop<FrameMul<U1>, MultiPass<U1>, Constant<U1>>,
            >,
        >,
    >,
>;

type ResamplerSpeed<K> = Pipe<
    Pipe<
        Pipe<
            Constant<U1>,
            Chain<
                K,
                Binop<
                    FrameAdd<U1>,
                    Pass,
                    Pipe<Var, Binop<FrameMul<U1>, MultiPass<U1>, Constant<U1>>>,
                >,
            >,
        >,
        Stack<Binop<FrameAdd<U1>, Pass, BandSum<K>>, Constant<U1>>,
    >,
    super::div::Div<S>,
>;

type NodesBus<K> = Pipe<
    Pipe<
        Pipe<Pipe<Pipe<MultiBus<K, NodeType>, ShelfType>, ButterLowpass<S, U1>>, Split<U2>>,
        Stack<Pass, ThrowCatchThrow>,
    >,
    MetroBusType<K>,
>;

type ProductionChain<K> = Pipe<Follow<S>, Stack<Resample<NodesBus<K>>, ThrowCatchCatch>>;

type OutputChain = Pipe<Stack<Pass, Delay>, Binop<FrameAdd<U1>, Pass, Pass>>;

pub type GroupType<K> = Pipe<Pipe<ResamplerSpeed<K>, ProductionChain<K>>, OutputChain>;

#[allow(clippy::unnecessary_cast)]
pub fn create_group_node<K>(
    config: &GroupConfig,
    group_handles: HashMap<NodeKey, InnerHandles>,
    values: &FineTunedValues,
) -> An<GroupType<K>>
where
    K: Size<S> + Size<NodeType>,
{
    let nodes = config.nodes.clone();
    debug_assert_eq!(
        nodes.len(),
        K::USIZE,
        "mismatched number of nodes and bus size"
    );
    debug_assert_eq!(
        nodes.len(),
        group_handles.len(),
        "mismatched number of nodes and handles"
    );
    let excitements_cell = RefCell::new(
        group_handles
            .values()
            .map(|h| h.siren_control.clone())
            .collect::<Vec<_>>(),
    );
    let bands_cell = RefCell::new(
        group_handles
            .values()
            .map(|h| h.band_control.clone())
            .collect::<Vec<_>>(),
    );

    let metro = metro_busi::<K>(config, &group_handles);

    let handles_cell = RefCell::new(group_handles);
    let values_clone = values.clone();
    let f_min = config
        .nodes
        .iter()
        .map(|n| n.frequency)
        .fold(f64::MAX, |acc, x| acc.min(x)) as S;
    let f_max = config
        .nodes
        .iter()
        .map(|n| n.frequency)
        .fold(f64::EPSILON.sqrt(), |acc, x| acc.max(x)) as S;

    log::debug!("Group Node: f_min = {}, f_max = {}", f_min, f_max);

    let gain: An<DbLin> = values.group_ls_gain_db.clone() >> super::db_lin::db_lin_converter();

    let shelf: An<ShelfType> =
        (pass() | constant((f_min * 0.75) as f32) | values.group_q.clone() | gain.clone())
            >> highshelf::<S>();

    let butter = butterpass_hz(f_max * 1.5);

    let coef = (1.0 / K::USIZE as S).powf(
        config
            .nodes
            .first()
            .map(|n| n.key.group() as S + 2.0)
            .unwrap_or_default(),
    ) / K::USIZE as S;

    log::info!("created group with speed coefficient: [{coef}]");

    let bands_sum: An<BandSum<K>> = constant(0.0)
        >> pipei::<K, _, _>(move |_| {
            let mut bands = bands_cell.borrow_mut();
            pass()
                + ((constant(1.0) - (var(&bands.pop().unwrap()) * constant(2.0)))
                    >> mul(-coef as f32))
        });
    let resampler_speed: An<ResamplerSpeed<K>> = constant(K::USIZE as f32)
        >> pipei::<K, _, _>(move |_| {
            let mut excitements = excitements_cell.borrow_mut();
            pass() + (var(&excitements.pop().unwrap().primary) >> mul(coef as f32))
        })
        >> ((pass() + bands_sum) | constant(K::USIZE as f32))
        >> super::div::div::<S>();

    let (throw, catch) = super::throw_catch::throw_catch(K::USIZE);

    let nodes_bus: An<NodesBus<K>> = busi::<K, _, _>(move |i| {
        let key = nodes[i as usize].key;
        let mut handles = handles_cell.borrow_mut();
        let handle = handles
            .remove(&key)
            .ok_or_else(|| format!("missing handle for node key: [{key:?}]"))
            .unwrap();

        create_node(&nodes[i as usize], handle, &values_clone)
    }) >> shelf
        >> butter
        >> split::<U2>()
        >> (pass() | throw)
        >> metro;

    // Get the follow response time valueF
    #[cfg(feature = "editor")]
    let follow_time = values.node_follow_response_time_s.value();
    #[cfg(not(feature = "editor"))]
    let follow_time = values.node_follow_response_time_s.value()[0];

    let production_chain: An<ProductionChain<K>> =
        follow::<S>(follow_time as S) >> (resample(nodes_bus) | catch);

    let group_d_cents = config.distance_cents();
    let group_delay = (1.0 / 1200.0) * group_d_cents;

    let output_chain: An<OutputChain> = (pass() | delay(group_delay)) >> (pass() + pass());

    resampler_speed >> production_chain >> output_chain
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::instrument::config_test_cases;
    use insta_fun::prelude::*;
    use test_log::test;
    use u_num_it::u_num_it;
    #[test]
    fn test_config_cases_groups() {
        #[cfg(feature = "editor")]
        let values = FineTunedValues::new(&FineTunedSharedValues::default());
        #[cfg(not(feature = "editor"))]
        let values = FineTunedValues::new();

        let mut snapshot_config = SnapshotConfigBuilder::default();
        snapshot_config.allow_abnormal_samples(true);
        snapshot_config.warm_up(WarmUp::Samples(8000));
        snapshot_config.num_samples(4000);
        let mut svg_config = SvgChartConfigBuilder::default();
        svg_config.show_grid(true);
        svg_config.chart_layout(Layout::Combined);
        svg_config.preserve_aspect_ratio(SvgPreserveAspectRatio::scale_to_fit());
        svg_config.show_grid(true);
        svg_config.chart_layout(Layout::Combined);

        for (config, layout) in config_test_cases() {
            for group in config.0 {
                let g_key = group.nodes.first().unwrap().key.group();

                let case_title = format!(
                    "config_test_case_group_{g_key}_{}x{}_{:?}",
                    layout.space.x, layout.space.y, layout.scale
                );

                let i_max = group.nodes.len();
                let group_handles =
                    HashMap::from_iter(group.nodes.iter().enumerate().map(|(i, node)| {
                        let handles = InnerHandles::default();
                        handles
                            .siren_control
                            .set_value((i as S / i_max as S, i as S / i_max as S));
                        (node.key, handles)
                    }));

                let mut net = Net::new(0, 1);

                let node_id = u_num_it!(
                    1..=11,
                    match i_max {
                        U => {
                            net.push(Box::new(create_group_node::<NumType>(
                                &group,
                                group_handles,
                                &values,
                            )))
                        }
                        _ => panic!("unsupported number of nodes in group: {i_max}"),
                    }
                );

                net.pipe_output(node_id);

                let audio_snapshot_config = snapshot_config
                    .clone()
                    .output_mode(WavOutput::Wav32)
                    .num_samples(44100)
                    .build()
                    .unwrap();

                assert_audio_unit_snapshot!(
                    &case_title,
                    net.clone(),
                    InputSource::None,
                    audio_snapshot_config
                );

                let svg_config = svg_config.clone().chart_title(&case_title).build().unwrap();
                let config = snapshot_config
                    .clone()
                    .output_mode(svg_config)
                    .build()
                    .unwrap();

                assert_audio_unit_snapshot!(net, config);
            }
        }
    }
}
