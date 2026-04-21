use std::sync::Arc;

use fundsp::{thingbuf::ThingBuf, Float, Real};
use num_complex::Complex;
use spectrum_analyzer::FrequencySpectrum;

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