use std::{
    array,
    marker::PhantomData,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicI32},
    },
};

use common::{tuner::Config as TunerConfig, instrument::Config as InstrumentConfig};
use fundsp::{Float, Real, prelude::*, thingbuf::ThingBuf, typenum::Unsigned};
use num_complex::Complex;
use ordered_float::{FloatCore, OrderedFloat};
use parking_lot::RwLock;
use spectrum_analyzer::{
    FrequencyLimit, FrequencySpectrum, samples_fft_to_spectrum, scaling, windows::hann_window,
};
use tokio::task::JoinHandle;

use crate::rt::ExcitementSource;

pub mod control {
    use fundsp::{Float, Real, prelude::shared, shared::Shared};
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
pub struct Excitor<S: Float + Real + FloatCore + 'static> {
    src: ExcitementSource,
    analyzer_job: Arc<JoinHandle<()>>,
    job_running: Arc<AtomicBool>,
    input_feed: Arc<ThingBuf<f32>>,
    excitement_feed: Arc<ThingBuf<Vec<Complex<S>>>>,
    tuner_config: Arc<RwLock<TunerConfig>>,
    instrument_config: Arc<RwLock<InstrumentConfig>>,
    frequency_limit: Arc<(AtomicI32, AtomicI32)>,
    _sample_type: PhantomData<S>,
}

impl<S: Float + Real + FloatCore + 'static> Excitor<S> {
    pub fn new(src: ExcitementSource, tuner_cfg: TunerConfig, instrument_cfg: InstrumentConfig) -> Self {
        let instrument_config = Arc::new(RwLock::new(instrument_cfg.clone()));
        let tuner_config = Arc::new(RwLock::new(tuner_cfg.clone()));
        let input_feed = Arc::new(ThingBuf::<f32>::new(FFT_WINDOW_SIZE + FFT_WINDOW_SIZE / 4));
        let excitement_feed = Arc::new(ThingBuf::<Vec<Complex<S>>>::new(2));
        let job_running = Arc::new(AtomicBool::new(true));
        let tuner_constraints = tuner_cfg.constraints();
        let frequency_limit = Arc::new((
            AtomicI32::new(
                tuner_constraints
                    .min_frequency
                    .map_or(-1, |l| l.round() as i32),
            ),
            AtomicI32::new(
                tuner_constraints
                    .max_frequency
                    .map_or(-1, |l| l.round() as i32),
            ),
        ));
        let analyzer_job = Arc::new({
            let job_running = job_running.clone();
            let input_feed = input_feed.clone();
            let excitement_feed = excitement_feed.clone();
            let config = tuner_config.clone();
            let frequency_limit = frequency_limit.clone();
            tokio::task::spawn(async move {
                while job_running.load(std::sync::atomic::Ordering::SeqCst) {
                    if input_feed.len() >= FFT_WINDOW_SIZE {
                        excitement_feed
                            .push_with(|place| {
                                let frequency_limit =
                                    Self::frequency_limit(frequency_limit.clone());
                                let config = config.read();

                                let spectrum = Self::analyze_fft_window(
                                    &input_feed,
                                    config.sample_rate as u32,
                                    frequency_limit,
                                );

                                let excitement = Self::excitement_from_spectrum(&spectrum, &config);

                                *place = excitement;
                            })
                            .unwrap_or_else(|e| {
                                log::error!("failed to push excitement data: {e}");
                            });
                    } else {
                        std::thread::yield_now();
                    }
                }
            })
        });
        Self {
            src,
            tuner_config,
            instrument_config,
            analyzer_job,
            job_running,
            input_feed,
            excitement_feed,
            frequency_limit,
            _sample_type: PhantomData,
        }
    }

    fn analyze_fft_window(
        input_feed: &Arc<ThingBuf<f32>>,
        sampling_rate: u32,
        frequency_limit: FrequencyLimit,
    ) -> FrequencySpectrum {
        let samples: [f32; FFT_WINDOW_SIZE] = array::from_fn(|_| {
            input_feed.pop().unwrap_or_else(|| {
                log::error!(
                    "input feed must contain at least FFT_WINDOW_SIZE samples when analyzing"
                );
                0.0
            })
        });
        let samples = hann_window(&samples);
        samples_fft_to_spectrum(
            &samples,
            sampling_rate,
            frequency_limit,
            Some(&scaling::combined(&[
                &scaling::scale_20_times_log10,
                &scaling::scale_to_zero_to_one,
            ])),
        )
        .unwrap_or_else(|e| {
            log::error!("failed to analyze FFT window: {e}");
            FrequencySpectrum::default()
        })
    }

    fn frequency_limit(frequency_limit: Arc<(AtomicI32, AtomicI32)>) -> FrequencyLimit {
        match (
            frequency_limit.0.load(std::sync::atomic::Ordering::SeqCst),
            frequency_limit.1.load(std::sync::atomic::Ordering::SeqCst),
        ) {
            (-1, -1) => FrequencyLimit::All,
            (min, -1) => FrequencyLimit::Min(min as f32),
            (-1, max) => FrequencyLimit::Max(max as f32),
            (min, max) => FrequencyLimit::Range(min as f32, max as f32),
        }
    }


    /// Computes the excitement values from spectrum analysis
    /// 
    /// Analyses the spectrum for direct sensors hit:
    /// - within min/max frequency bounds of the sensor
    /// - within min/max magnitude bounds of the sensor
    fn excitement_from_spectrum(
        spectrum: &FrequencySpectrum,
        config: &TunerConfig,
    ) -> Vec<Complex<S>> {
        let resolution = spectrum.frequency_resolution();

        config
            .sensor_data
            .iter()
            .map(|sensor_cfg| {
                let probes = sensor_cfg.probes(resolution);
                let num_probes = probes.len();

                let center = (num_probes as f32 - 1.0) / 2.0;
                probes
                    .into_iter()
                    .enumerate()
                    .filter_map(|(i, search_fr)| {
                        let (freq, freq_value) = spectrum.freq_val_closest(search_fr);
                        if (sensor_cfg.min_frequency..sensor_cfg.max_frequency)
                            .contains(&freq.val())
                            && (sensor_cfg.min_magnitude..sensor_cfg.max_magnitude)
                                .contains(&freq_value.val())
                        {
                            let real = freq_value.val();
                            let imaginary = if center > 0.0 {
                                (i as f32 - center).abs() / center
                            } else {
                                0.0
                            };
                            Some(Complex::new(S::from_f32(real), S::from_f32(imaginary)))
                        } else {
                            None
                        }
                    })
                    .max_by_key(|c| OrderedFloat(c.re))
                    .unwrap_or(Complex::new(convert(0_f32), convert(0_f32)))
            })
            .collect()
    }

    /// Analyses the feedback data of the nodes (aka previous tick)
    /// 
    /// Computes the sympathetic resonance effect on feedback data:
    /// - 
    fn excitement_from_feedback(
        feedback_data: &[f32],
        tuner_config: &TunerConfig,
        instrument_config: &InstrumentConfig
    ) -> Vec<Complex<S>> {

        tuner_config.sensor_data.iter().map(|sensor_cfg| {
            let node_cfg = instrument_config.get_node(&sensor_cfg.key).expect("instrument and tuner data configuration mismatch");
            let index = instrument_config.get_node_index(&sensor_cfg.key).expect("instrument and tuner data configuration mismatch");
            let sample = feedback_data.get(index).expect("feedback data length must match the number of nodes in the instrument configuration");
            

        }).collect()
    }
}

impl<S: Float + Real + FloatCore + 'static> Drop for Excitor<S> {
    fn drop(&mut self) {
        self.job_running
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

impl<S: Float + Real + FloatCore + 'static> AudioUnit for Excitor<S> {
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
        (match self.src {
            ExcitementSource::Mic => 1,
            _ => 0,
        }) + self.tuner_config.read().sensor_data.len()
            * <super::feedback_pass::FeedbackCatch as AudioNode>::Outputs::USIZE
    }

    fn outputs(&self) -> usize {
        self.tuner_config.read().sensor_data.len()
            * <super::node::NodeController<S> as AudioNode>::Inputs::USIZE
    }

    fn route(
        &mut self,
        input: &fundsp::prelude::SignalFrame,
        _frequency: f64,
    ) -> fundsp::prelude::SignalFrame {
        // Analysis/control path implies at least one FFT-window delay.
        fundsp::signal::Routing::Arbitrary(FFT_WINDOW_SIZE as f64).route(input, self.outputs())
    }

    fn get_id(&self) -> u64 {
        EXCITOR_ID
    }

    fn footprint(&self) -> usize {
        core::mem::size_of::<Self>()
    }
}
