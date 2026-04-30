use std::{
    marker::PhantomData,
    sync::{atomic::AtomicBool, Arc},
};

use common::tuner::Config as TunerConfig;
use fundsp::{
    prelude::{AudioNode, AudioUnit},
    thingbuf::{mpsc::Sender, ThingBuf},
    typenum::Unsigned,
    Float, Real,
};
use num_complex::Complex;
use spectrum_analyzer::FrequencySpectrum;
use tokio::task::JoinHandle;

use crate::rt::ExcitementSource;

pub mod control {
    use fundsp::{prelude::shared, shared::Shared, Float, Real};
    use num_complex::Complex;

    #[derive(Clone)]
    pub struct Control {
        pub real: Shared,
        pub imaginary: Shared,
    }

    impl Control {
        pub fn new(real: Shared, imaginary: Shared) -> Self {
            Self { real, imaginary }
        }

        pub fn value<S: Real + Float>(&self) -> Complex<S> {
            Complex::new(self.primary_value(), self.secondary_value())
        }

        pub fn primary_value<S: Real + Float>(&self) -> S {
            S::from_f32(self.real.value())
        }

        pub fn secondary_value<S: Real + Float>(&self) -> S {
            S::from_f32(self.imaginary.value())
        }

        pub fn set_value<S: Real + Float>(&self, (real, imaginary): (S, S)) {
            self.real.set_value(real.to_f32());
            self.imaginary.set_value(imaginary.to_f32());
        }

        pub fn reset(&self) {
            self.real.set_value(0.0);
            self.imaginary.set_value(0.0);
        }
    }

    impl Default for Control {
        fn default() -> Self {
            Self {
                real: shared(0.0),
                imaginary: shared(0.0),
            }
        }
    }
}

pub type SpectrumBuffer = Arc<ThingBuf<Arc<FrequencySpectrum>>>;

pub const FFT_WINDOW_SIZE: usize = 8192;

pub fn snapshot_control<S: Real + Float>(control: &control::Control) -> Complex<S> {
    control.value()
}

const EXCITOR_ID: u64 = crate::util::hash_str(concat!(module_path!(), "::Excitor"));

#[derive(Clone)]
pub struct Excitor<S: Float + Real> {
    src: ExcitementSource,
    job: Arc<JoinHandle<()>>,
    job_running: Arc<AtomicBool>,
    data_feed: Sender<S>,
    config: Arc<TunerConfig>,
    _sample_type: PhantomData<S>,
}

impl<S: Float + Real> AudioUnit for Excitor<S> {
    fn tick(&mut self, input: &[f32], output: &mut [f32]) {
        todo!()
    }

    fn process(
        &mut self,
        size: usize,
        input: &fundsp::prelude::BufferRef,
        output: &mut fundsp::prelude::BufferMut,
    ) {
        todo!()
    }

    fn inputs(&self) -> usize {
        match self.src {
            ExcitementSource::Mic => 1,
            _ => 0,
        }
    }

    fn outputs(&self) -> usize {
        self.config.sensor_data.len() * <super::node::NodeController<S> as AudioNode>::Inputs::USIZE
    }

    fn route(
        &mut self,
        input: &fundsp::prelude::SignalFrame,
        frequency: f64,
    ) -> fundsp::prelude::SignalFrame {
        todo!()
    }

    fn get_id(&self) -> u64 {
        EXCITOR_ID
    }

    fn footprint(&self) -> usize {
        todo!()
    }
}
