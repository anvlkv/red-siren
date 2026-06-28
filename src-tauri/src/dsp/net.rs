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
        let mut intermediate = A::output_buffer::<S>();
        assert_eq!(
            intermediate.len(),
            B::input_buffer::<S>().len(),
            "Unit buffers' sizes must match for piping"
        );

        Box::new(
            move |tick_data: &NetTickData, input: &[S], output: &mut [S]| {
                a_backend.process(tick_data, input, &mut intermediate);
                b_backend.process(tick_data, &intermediate, output);
            },
        )
    }

    fn output_buffer<S: Float + Send + Sync + 'static>() -> Vec<S> {
        B::output_buffer::<S>()
    }

    fn input_buffer<S: Float + Send + Sync + 'static>() -> Vec<S> {
        A::input_buffer::<S>()
    }
}

pub trait DspUnit
where
    Self: Sized,
{
    fn backend<S: Float + Send + Sync + 'static>(&self)
        -> Box<dyn DspUnitBackend<S> + Send + Sync>;

    fn output_buffer<S: Float + Send + Sync + 'static>() -> Vec<S>;

    fn input_buffer<S: Float + Send + Sync + 'static>() -> Vec<S>;

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
        (self)(tick_data, input, output)
    }
}
