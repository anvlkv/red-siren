use std::{marker::PhantomData, ops::Mul};

use common::instrument::{BandConfig, NodeConfig};
use fundsp::{numeric_array::ArrayLength, prelude::*};

use crate::values::FineTunedValues;

const BAND_NODE_ID: u64 = crate::util::hash_str(concat!(module_path!(), "::Band")); // Unique identifier for the Band node

#[derive(Clone)]
pub struct Band<
    S: Real + Float + 'static,
    X: AudioNode<Outputs = XO, Inputs = XI> + 'static,
    N: Size<S> + Size<X>,
    XI: Size<S> + Size<X>,
    XO: Size<S> + Size<X>,
    XON: Size<S> + Size<X>,
    XIN: Size<S> + Size<X>,
> where
    XI: Mul<N, Output = XIN>,
    <XI as Mul<N>>::Output: ArrayLength + Send + Sync,
    XO: Mul<N, Output = XON>,
    <XO as Mul<N>>::Output: ArrayLength + Send + Sync,
{
    inner: Net,
    _config: BandConfig,
    _sample_type: PhantomData<S>,
    _num_nodes: PhantomData<N>,
    _num_inputs_per_node: PhantomData<XI>,
    _num_outputs_per_node: PhantomData<XO>,
    _num_inputs: PhantomData<XIN>,
    _num_outputs: PhantomData<XON>,
    _node_type: PhantomData<X>,
}

impl<
        S: Real + Float + 'static,
        X: AudioNode<Outputs = XO, Inputs = XI> + 'static,
        N: Size<S> + Size<X>,
        XI: Size<S> + Size<X>,
        XO: Size<S> + Size<X>,
        XON: Size<S> + Size<X>,
        XIN: Size<S> + Size<X>,
    > Band<S, X, N, XI, XO, XON, XIN>
where
    XI: Mul<N, Output = XIN>,
    <XI as Mul<N>>::Output: ArrayLength + Send + Sync,
    XO: Mul<N, Output = XON>,
    <XO as Mul<N>>::Output: ArrayLength + Send + Sync,
{
    pub fn new<G: Fn(NodeConfig) -> An<X>>(
        config: BandConfig,
        generator: G,
        values: &FineTunedValues,
    ) -> Self {
        let mut net = Net::new(N::USIZE * X::Inputs::USIZE as usize, 1);
        let center_freq =
            config.nodes.iter().map(|n| n.frequency).sum::<f64>() / config.nodes.len() as f64;
        let max_freq = config.nodes.last().map(|n| n.frequency).unwrap_or(0.0);
        let min_freq = config.nodes.first().map(|n| n.frequency).unwrap_or(0.0);

        let band_stack = net.push(Box::new(
            stacki::<N, X, _>(|i| generator(config.nodes[i as usize])) >> join::<XON>(),
        ));

        let filter_input = split::<U3>()
            >> (multipass::<U3>()
                | constant(Frame::<f32, U3>::from([
                    min_freq as f32,
                    center_freq as f32,
                    max_freq as f32,
                ]))
                | values.band_bell_q.clone()
                | values.band_shelf_q.clone()
                | values.band_bell_gain.clone()
                | values.band_shelf_gain.clone());

        let filter_mapper = net.push(Box::new(
            filter_input
                >> An(Map::new(
                    |frame: &Frame<f32, U10>| {
                        let audio_1 = frame[0];
                        let audio_2 = frame[1];
                        let audio_3 = frame[2];
                        let min_freq = frame[3];
                        let center_freq = frame[4];
                        let max_freq = frame[5];
                        let bell_q = frame[6];
                        let shelf_q = frame[7];
                        let bell_gain = frame[8];
                        let shelf_gain = frame[9];
                        Frame::<f32, U12>::from([
                            audio_1,
                            center_freq,
                            bell_q,
                            bell_gain, // bell filter inputs
                            audio_2,
                            max_freq,
                            shelf_q,
                            shelf_gain, // high shelf filter inputs
                            audio_3,
                            min_freq,
                            shelf_q,
                            shelf_gain, // low shelf filter inputs
                        ])
                    },
                    Routing::Split,
                )),
        ));

        let filter_stack = net.push(Box::new(
            (bell::<S>() | highshelf::<S>() | lowshelf::<S>()) >> join::<U3>(),
        ));

        net.pipe_input(band_stack);
        net.pipe_all(band_stack, filter_mapper);
        net.pipe_all(filter_mapper, filter_stack);
        net.pipe_output(filter_stack);

        Self {
            inner: net,
            _config: config,
            _sample_type: PhantomData,
            _num_nodes: PhantomData,
            _num_inputs_per_node: PhantomData,
            _num_outputs_per_node: PhantomData,
            _num_inputs: PhantomData,
            _num_outputs: PhantomData,
            _node_type: PhantomData,
        }
    }
}

impl<
        S: Real + Float + 'static,
        X: AudioNode<Outputs = XO, Inputs = XI> + 'static,
        N: Size<S> + Size<X>,
        XI: Size<S> + Size<X>,
        XO: Size<S> + Size<X>,
        XON: Size<S> + Size<X>,
        XIN: Size<S> + Size<X>,
    > AudioNode for Band<S, X, N, XI, XO, XON, XIN>
where
    XI: Mul<N, Output = XIN>,
    <XI as Mul<N>>::Output: ArrayLength + Send + Sync,
    XO: Mul<N, Output = XON>,
    <XO as Mul<N>>::Output: ArrayLength + Send + Sync,
{
    const ID: u64 = BAND_NODE_ID;
    type Inputs = XIN;
    type Outputs = XON;

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let mut output = Frame::<f32, Self::Outputs>::default();
        self.inner.tick(input, &mut output);
        output
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        self.inner.process(size, input, output);
    }

    fn reset(&mut self) {
        self.inner.reset();
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.inner.set_sample_rate(sample_rate);
    }

    fn set_hash(&mut self, hash: u64) {
        self.inner.set_hash(hash);
    }
}

pub fn create_band_node<
    S: Real + Float + 'static,
    X: AudioNode<Outputs = XO, Inputs = XI> + 'static,
    N: Size<S> + Size<X>,
    XI: Size<S> + Size<X>,
    XO: Size<S> + Size<X>,
    XON: Size<S> + Size<X>,
    XIN: Size<S> + Size<X>,
    G: Fn(NodeConfig) -> An<X>,
>(
    config: BandConfig,
    generator: G,
    values: &FineTunedValues,
) -> An<Band<S, X, N, XI, XO, XON, XIN>>
where
    XI: Mul<N, Output = XIN>,
    <XI as Mul<N>>::Output: ArrayLength + Send + Sync,
    XO: Mul<N, Output = XON>,
    <XO as Mul<N>>::Output: ArrayLength + Send + Sync,
{
    An(Band::new(config, generator, values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{chart_snapshot_config, constant_input_by_channel};
    use common::instrument::BandChannel;
    use fundsp::prelude32::sine_hz;
    use insta_fun::prelude::*;

    fn make_band_config() -> BandConfig {
        BandConfig {
            channel: BandChannel::Left,
            nodes: vec![NodeConfig::new_test_node(220.0)],
        }
    }

    fn snapshot_config(num_samples: usize) -> SnapshotConfig {
        chart_snapshot_config(num_samples, &["Audio In"], &["Band Out"])
    }

    fn band_under_test(
    ) -> An<Band<f32, Binop<FrameMul<U1>, Pipe<Constant<U1>, Sine<f32>>, Pass>, U1, U1, U1, U1, U1>>
    {
        let config = make_band_config();
        let values = FineTunedValues::new();
        let func = |node: NodeConfig| sine_hz(node.frequency as f32) * pass();
        create_band_node::<
            f32,
            Binop<FrameMul<U1>, Pipe<Constant<U1>, Sine<f32>>, Pass>,
            U1,
            U1,
            U1,
            U1,
            U1,
            _,
        >(config, func, &values)
    }

    #[test]
    fn band_sine_stack_with_steady_input() {
        let band = band_under_test();
        assert_audio_unit_snapshot!(
            "band_sine_stack_with_steady_input",
            band,
            constant_input_by_channel(256, &[1.0]),
            snapshot_config(256)
        );
    }

    #[test]
    fn band_sine_stack_with_impulse_input() {
        let band = band_under_test();
        assert_audio_unit_snapshot!(
            "band_sine_stack_with_impulse_input",
            band,
            InputSource::impulse(),
            snapshot_config(256)
        );
    }
}
