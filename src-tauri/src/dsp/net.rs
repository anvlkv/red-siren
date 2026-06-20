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
