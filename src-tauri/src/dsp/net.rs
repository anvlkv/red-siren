use num_traits::Float;

use crate::audio_runtime::SampleType;

pub trait DspNetwork {
    fn backend(&self) -> Box<dyn DspNetworkBackend + Send + Sync>;

    fn set_sample_rate(&self, sample_rate: u32);

    fn set_sample_type(&self, sample_type: SampleType);

    fn fade_in(&self, duration: f64);

    fn fade_out(&self, duration: f64);
}

pub trait DspNetworkBackend {
    fn process(&mut self, input: &[f32], output: &mut [f32]);
}

impl<F> DspNetworkBackend for F
where
    F: FnMut(&[f32], &mut [f32]) + Send + Sync,
{
    fn process(&mut self, input: &[f32], output: &mut [f32]) {
        (self)(input, output)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NetTickData {
    pub time: f64,
    pub sample_rate: u32,
}

pub struct PipeUnit<'rt, A: DspUnit, B: DspUnit> {
    a: &'rt A,
    b: &'rt B,
}

impl<'rt, A: DspUnit, B: DspUnit> DspUnit for PipeUnit<'rt, A, B> {
    fn backend<S: Float + Send + Sync + 'static>(
        &self,
    ) -> Box<dyn DspUnitBackend<S> + Send + Sync> {
        let mut a_backend = self.a.backend::<S>();
        let mut b_backend = self.b.backend::<S>();
        let mut intermediate = A::output_frame::<S>();
        assert_eq!(
            intermediate.len(),
            B::input_frame::<S>().len(),
            "Unit frame' sizes must match for piping"
        );

        Box::new(
            move |tick_data: &NetTickData, input: &[S], output: &mut [S]| {
                a_backend.process(tick_data, input, &mut intermediate);
                b_backend.process(tick_data, &intermediate, output);
            },
        )
    }

    fn output_frame<S: Float + Send + Sync + 'static>() -> Vec<S> {
        B::output_frame::<S>()
    }

    fn input_frame<S: Float + Send + Sync + 'static>() -> Vec<S> {
        A::input_frame::<S>()
    }
}

pub trait DspUnit
where
    Self: Sized,
{
    fn backend<S: Float + Send + Sync + 'static>(&self)
        -> Box<dyn DspUnitBackend<S> + Send + Sync>;

    fn output_frame<S: Float + Send + Sync + 'static>() -> Vec<S>;

    fn input_frame<S: Float + Send + Sync + 'static>() -> Vec<S>;

    fn pipe<'rt, U: DspUnit>(&'rt self, next: &'rt U) -> PipeUnit<'rt, Self, U> {
        PipeUnit { a: self, b: next }
    }
}

pub trait DspUnitBackend<S: Float + Send + Sync + 'static> {
    fn process(&mut self, tick_data: &NetTickData, input: &[S], output: &mut [S]);
}

impl<F, S: Float + Send + Sync + 'static> DspUnitBackend<S> for F
where
    F: FnMut(&NetTickData, &[S], &mut [S]) + Send + Sync,
{
    fn process(&mut self, tick_data: &NetTickData, input: &[S], output: &mut [S]) {
        if cfg!(debug_assertions) && input.iter().any(|f| f.is_infinite() || f.is_nan()) {
            let input = input
                .iter()
                .map(|s| s.to_f32().unwrap())
                .collect::<Vec<_>>();
            log::warn!(
                "DspUnitBackend input contains non-finite values: {:?}",
                input,
            );
        }
        (self)(tick_data, input, output);
        if cfg!(debug_assertions) && output.iter().any(|f| f.is_infinite() || f.is_nan()) {
            let output = output
                .iter()
                .map(|s| s.to_f32().unwrap())
                .collect::<Vec<_>>();
            log::warn!(
                "DspUnitBackend output contains non-finite values: {:?}",
                output,
            );
        }
    }
}
