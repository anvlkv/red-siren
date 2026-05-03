use std::{
    array,
    marker::PhantomData,
    sync::{
        atomic::{AtomicBool, AtomicI32},
        Arc,
    },
    thread::JoinHandle,
};

use common::{
    instrument::{Config as InstrumentConfig, ResonanceModel},
    tuner::Config as TunerConfig,
};
use fastrand::Rng;
use fundsp::{prelude::*, thingbuf::ThingBuf, typenum::Unsigned, Float, Real};
use num_complex::Complex;
use ordered_float::{self, FloatCore, OrderedFloat};
use parking_lot::{Mutex, RwLock};
use spectrum_analyzer::{
    samples_fft_to_spectrum, scaling, windows::hann_window, FrequencyLimit, FrequencySpectrum,
};

use crate::rt::ExcitementSource;

pub mod control;

pub type SpectrumBuffer = Arc<ThingBuf<Arc<FrequencySpectrum>>>;
pub type ExcitementData<S> = Complex<OrderedFloat<S>>;

const EXCITOR_ID: u64 = crate::util::hash_str(concat!(module_path!(), "::Excitor"));

#[cfg(not(test))]
pub const FFT_WINDOW_SIZE: usize = 8192;

#[cfg(test)]
pub const FFT_WINDOW_SIZE: usize = 256;

/// Associates a complex type with its scalar component type.
/// This trait ensures that C is properly derived from S at the type level.
pub trait ComplexFor: Clone + Send + Sync {
    type Scalar;
}

impl<T: Clone + Send + Sync> ComplexFor for Complex<T> {
    type Scalar = T;
}

struct AnalyzerRuntime {
    running: AtomicBool,
    job: Mutex<Option<JoinHandle<()>>>,
}

impl AnalyzerRuntime {
    fn new() -> Self {
        Self {
            running: AtomicBool::new(true),
            job: Mutex::new(None),
        }
    }
}

#[derive(Clone)]
pub struct Excitor<S: Float + Real + ordered_float::Float + 'static, C = ExcitementData<S>> {
    src: ExcitementSource,
    analyzer_runtime: Arc<AnalyzerRuntime>,
    input_feed: Arc<ThingBuf<f32>>,
    analyzer_excitement_feed: Arc<ThingBuf<Vec<C>>>,
    spectrum_buffer: SpectrumBuffer,
    summary_excitement_snapshot: Arc<RwLock<Vec<ExcitementData<S>>>>,
    tuner_config: Arc<RwLock<TunerConfig>>,
    instrument_config: Arc<RwLock<InstrumentConfig>>,
    frequency_limit: Arc<(AtomicI32, AtomicI32)>,
    _sample_type: PhantomData<S>,
    rng: Arc<Mutex<Rng>>,
}

impl<S: Float + Real + ordered_float::Float + FloatCore + 'static> Excitor<S> {
    pub fn new(
        src: ExcitementSource,
        tuner_cfg: TunerConfig,
        instrument_cfg: InstrumentConfig,
        rng: Rng,
    ) -> Self {
        let instrument_config = Arc::new(RwLock::new(instrument_cfg.clone()));
        let tuner_config = Arc::new(RwLock::new(tuner_cfg.clone()));
        let input_feed = Arc::new(ThingBuf::<f32>::new(FFT_WINDOW_SIZE + MAX_BUFFER_SIZE));
        let analyzer_excitement_feed =
            Arc::new(ThingBuf::<Vec<ExcitementData<S>>>::new(MAX_BUFFER_SIZE));
        let spectrum_buffer = SpectrumBuffer::new(ThingBuf::new(MAX_BUFFER_SIZE));
        let analyzer_runtime = Arc::new(AnalyzerRuntime::new());
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
        let analyzer_job = Self::start_analyzer_job(
            analyzer_runtime.clone(),
            input_feed.clone(),
            analyzer_excitement_feed.clone(),
            spectrum_buffer.clone(),
            tuner_config.clone(),
            frequency_limit.clone(),
        );
        *analyzer_runtime.job.lock() = Some(analyzer_job);
        let summary_excitement_snapshot = Arc::new(RwLock::new(Vec::new()));

        Self {
            src,
            tuner_config,
            instrument_config,
            analyzer_runtime,
            input_feed,
            analyzer_excitement_feed,
            spectrum_buffer,
            summary_excitement_snapshot,
            frequency_limit,
            rng: Arc::new(Mutex::new(rng)),
            _sample_type: PhantomData,
        }
    }

    pub fn update_config(
        &self,
        new_tuner_config: TunerConfig,
        new_instrument_config: InstrumentConfig,
    ) {
        *self.tuner_config.write() = new_tuner_config.clone();
        let tuner_constraints = new_tuner_config.constraints();
        self.frequency_limit.0.store(
            tuner_constraints
                .min_frequency
                .map_or(-1, |l| l.round() as i32),
            std::sync::atomic::Ordering::SeqCst,
        );
        self.frequency_limit.1.store(
            tuner_constraints
                .max_frequency
                .map_or(-1, |l| l.round() as i32),
            std::sync::atomic::Ordering::SeqCst,
        );
        *self.instrument_config.write() = new_instrument_config;
    }

    pub fn update_frequency_limit(&self, min_frequency: Option<f32>, max_frequency: Option<f32>) {
        self.frequency_limit.0.store(
            min_frequency.map_or(-1, |l| l.round() as i32),
            std::sync::atomic::Ordering::SeqCst,
        );
        self.frequency_limit.1.store(
            max_frequency.map_or(-1, |l| l.round() as i32),
            std::sync::atomic::Ordering::SeqCst,
        );
    }

    pub fn spectrum_feed(&self) -> SpectrumBuffer {
        self.spectrum_buffer.clone()
    }

    pub fn summary_snapshot(&self) -> Arc<RwLock<Vec<ExcitementData<S>>>> {
        self.summary_excitement_snapshot.clone()
    }

    fn start_analyzer_job(
        analyzer_runtime: Arc<AnalyzerRuntime>,
        input_feed: Arc<ThingBuf<f32>>,
        excitement_feed: Arc<ThingBuf<Vec<ExcitementData<S>>>>,
        spectrum_buffer: SpectrumBuffer,
        tuner_config: Arc<RwLock<TunerConfig>>,
        frequency_limit: Arc<(AtomicI32, AtomicI32)>,
    ) -> JoinHandle<()> {
        std::thread::spawn(move || {
            while analyzer_runtime
                .running
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                if input_feed.len() >= FFT_WINDOW_SIZE {
                    let config = tuner_config.read();
                    let frequency_limit = Self::frequency_limit(frequency_limit.clone());

                    let spectrum = Self::analyze_fft_window(
                        &input_feed,
                        config.sample_rate as u32,
                        frequency_limit,
                    );

                    let excitement = Self::excitement_from_spectrum(&spectrum, &config);
                    excitement_feed.push(excitement).unwrap_or_else(|e| {
                        log::error!("failed to push excitement data: {e}");
                    });
                    spectrum_buffer
                        .push(Arc::new(spectrum))
                        .unwrap_or_else(|e| {
                            log::error!("failed to push spectrum data: {e}");
                        });
                } else {
                    std::thread::yield_now();
                }
            }
        })
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
            Some(&scaling::scale_to_zero_to_one),
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
    ) -> Vec<ExcitementData<S>> {
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
                            Some(Complex::new(
                                OrderedFloat(S::from_f32(real)),
                                OrderedFloat(S::from_f32(imaginary)),
                            ))
                        } else {
                            None
                        }
                    })
                    .max_by(|a, b| {
                        // Compare by magnitude since Complex numbers don't have a natural ordering
                        let mag_a = a.norm_sqr();
                        let mag_b = b.norm_sqr();
                        mag_a
                            .partial_cmp(&mag_b)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .unwrap_or(Complex::new(
                        OrderedFloat(convert(0_f32)),
                        OrderedFloat(convert(0_f32)),
                    ))
            })
            .collect()
    }

    /// Analyses the feedback data of the nodes (aka previous tick).
    ///
    /// Computes a harmonic-likeness sympathetic resonance term from instrument-derived
    /// resonance parameters.
    fn excitement_from_feedback(
        feedback_data: &[f32],
        tuner_config: &TunerConfig,
        instrument_config: &InstrumentConfig,
        resonance_model: ResonanceModel,
    ) -> Vec<ExcitementData<S>> {
        let harmonics = std::cmp::max(resonance_model.harmonics, 1);
        let gamma = resonance_model.gamma.max(f32::EPSILON);

        tuner_config
            .sensor_data
            .iter()
            .map(|sensor_cfg| {
                let node_cfg = instrument_config
                    .get_node(&sensor_cfg.key)
                    .expect("instrument and tuner data configuration mismatch");
                let index = instrument_config
                    .get_node_index(&sensor_cfg.key)
                    .expect("instrument and tuner data configuration mismatch");
                let sample = feedback_data.get(index).expect(
                    "feedback data length must match the number of nodes in the instrument configuration",
                );

                let min_f = sensor_cfg.min_frequency.max(1.0);
                let max_f = sensor_cfg.max_frequency.max(min_f + f32::EPSILON);
                let center_f = 0.5 * (min_f + max_f);
                let half_bw = 0.5 * (max_f - min_f).max(f32::EPSILON);
                let base_f = node_cfg.frequency as f32;

                let best_coupling = (1..=harmonics)
                    .map(|h| {
                        let harmonic_f = base_f * h as f32;
                        let detune_norm =
                            ((harmonic_f - center_f).abs() / center_f.max(1.0)).clamp(0.0, 4.0);
                        1.0 / (1.0 + (detune_norm / gamma).powi(2))
                    })
                    .fold(0.0_f32, f32::max)
                    .clamp(0.0, 1.0);

                let physical_q = ((node_cfg.w_kg as f32).recip() * 10.0).clamp(0.2, 1.0);
                let strength = (sample.abs() * best_coupling * physical_q).clamp(0.0, 1.0);

                let nearest_detune = ((base_f - center_f).abs() / half_bw).clamp(0.0, 1.0);

                Complex::new(
                    OrderedFloat(S::from_f32(strength)),
                    OrderedFloat(S::from_f32(nearest_detune)),
                )
            })
            .collect()
    }

    /// Combines the excitement from spectrum analysis and feedback data to produce the final excitement values for each sensor.
    ///
    /// Blending weights are derived from the resonance model's physical bandwidth:
    /// - Heavier instruments (higher gamma) have broader resonance curves and rely more on feedback
    /// - Lighter instruments (lower gamma) have narrower resonance curves and rely more on direct spectrum
    fn summary_excitement(
        spectrum: Vec<ExcitementData<S>>,
        feedback: Vec<ExcitementData<S>>,
        manual: Vec<ExcitementData<S>>,
        resonance_model: ResonanceModel,
    ) -> Vec<ExcitementData<S>> {
        // Derive blending weights from resonance bandwidth:
        // Map gamma [MIN_GAMMA, MAX_GAMMA] to feedback weight [0.15, 0.45]
        // This reflects that heavier instruments (higher gamma) have more sympathetic resonance
        let gamma_norm = ((resonance_model.gamma as f64
            - common::instrument::RESONANCE_MIN_GAMMA as f64)
            / (common::instrument::RESONANCE_MAX_GAMMA as f64
                - common::instrument::RESONANCE_MIN_GAMMA as f64))
            .clamp(0.0, 1.0);
        let w_f_f64 = 0.15 + gamma_norm * 0.3; // [0.15, 0.45]
        let w_s_f64 = 1.0 - w_f_f64;

        let w_s = OrderedFloat(S::from_f64(w_s_f64));
        let w_f = OrderedFloat(S::from_f64(w_f_f64));

        let primary_src = if !manual.is_empty() { manual } else { spectrum };

        primary_src
            .into_iter()
            .zip(feedback)
            .map(|(s, f)| s * w_s + f * w_f)
            .collect()
    }

    /// Computes one per-sample excitement frame from analyzer, feedback, and control inputs.
    fn compute_excitement_frame(
        analyzer: Vec<ExcitementData<S>>,
        feedback_input: &[f32],
        control_input: &[f32],
        tuner_config: &TunerConfig,
        instrument_config: &InstrumentConfig,
        resonance_model: ResonanceModel,
    ) -> Vec<ExcitementData<S>> {
        let feedback_excitement = Self::excitement_from_feedback(
            feedback_input,
            tuner_config,
            instrument_config,
            resonance_model,
        );
        let manual_excitement = Self::excitement_from_control(control_input);
        Self::summary_excitement(
            analyzer,
            feedback_excitement,
            manual_excitement,
            resonance_model,
        )
    }

    fn update_summary_excitement_snapshot(&self, excitement: &[ExcitementData<S>]) {
        if let Some(mut snapshot) = self.summary_excitement_snapshot.try_write() {
            *snapshot = excitement.to_vec();
        }
    }

    fn write_interleaved_output(excitement: &[ExcitementData<S>], output: &mut [f32]) {
        for (exc, pair) in excitement.iter().zip(output.chunks_exact_mut(2)) {
            pair[0] = exc.re.to_f32();
            pair[1] = exc.im.to_f32();
        }
    }

    fn write_buffer_sample_output(
        excitement: &[ExcitementData<S>],
        output: &mut fundsp::prelude::BufferMut,
        sample: usize,
    ) {
        for (index, exc) in excitement.iter().enumerate() {
            output.set_f32(index * 2, sample, exc.re.to_f32());
            output.set_f32(index * 2 + 1, sample, exc.im.to_f32());
        }
    }

    /// Derives excitement values directly from the control input (for manual excitation).
    fn excitement_from_control(control: &[f32]) -> Vec<ExcitementData<S>> {
        control
            .chunks(2)
            .map(|chunk| {
                let real = chunk.get(0).cloned().unwrap_or(0.0);
                let imaginary = chunk.get(1).cloned().unwrap_or(0.0);
                Complex::new(
                    OrderedFloat(S::from_f32(real)),
                    OrderedFloat(S::from_f32(imaginary)),
                )
            })
            .collect()
    }

    fn entropy_sample(&self) -> f32 {
        let mut rng = self.rng.lock();
        convert(rng.f64() * 2.0 - 1.0) // Random value in [-1.0, 1.0]
    }

    /// Fills the provided buffer with random values in the range [-1.0, 1.0] for entropy excitation.
    fn fill_entropy(&self, buffer: &mut [f32]) {
        buffer.iter_mut().for_each(|sample| {
            *sample = self.entropy_sample();
        });
    }

    /// Pushes input to the feed based on excitement source.
    fn push_input_feed(&self, input: &fundsp::prelude::BufferRef, sample: usize) {
        match self.src {
            ExcitementSource::Mic => {
                self.input_feed
                    .push(input.at_f32(0, sample))
                    .unwrap_or_else(|e| {
                        log::warn!("failed to push mic input sample: {e}");
                    });
            }
            ExcitementSource::Entropy => {
                let entropy_sample = self.entropy_sample();
                self.input_feed.push(entropy_sample).unwrap_or_else(|e| {
                    log::warn!("failed to push entropy sample: {e}");
                });
            }
            ExcitementSource::Manual => {
                // No input feed for manual
            }
        }
    }
}

impl<S: Float + Real + ordered_float::Float + 'static, C> Drop for Excitor<S, C> {
    fn drop(&mut self) {
        if Arc::strong_count(&self.analyzer_runtime) == 1 {
            self.analyzer_runtime
                .running
                .store(false, std::sync::atomic::Ordering::SeqCst);
            if let Some(job) = self.analyzer_runtime.job.lock().take() {
                match job.join() {
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("joining analyzer job failed during drop: {e:?}")
                    }
                }
            }
        }
    }
}

impl<S: Float + Real + ordered_float::Float + FloatCore + 'static> AudioUnit
    for Excitor<S, ExcitementData<S>>
{
    fn tick(&mut self, input: &[f32], output: &mut [f32]) {
        let mut entropy_input = [0_f32];

        let (mic_input, feedback_input, control_input) = match self.src {
            ExcitementSource::Mic => (&input[..1], &input[1..], &input[..0]),
            ExcitementSource::Entropy => {
                self.fill_entropy(entropy_input.as_mut_slice());
                (
                    entropy_input.as_slice(),
                    &input[..self.tuner_config.read().sensor_data.len()],
                    &input[..0],
                )
            }
            ExcitementSource::Manual => (
                &input[..0],
                &input[..self.tuner_config.read().sensor_data.len()],
                &input[self.tuner_config.read().sensor_data.len()..],
            ),
        };
        for &sample in mic_input {
            self.input_feed.push(sample).unwrap_or_else(|e| {
                log::warn!("failed to push mic input sample: {e}");
            });
        }

        let tuner_config = self.tuner_config.read();
        let instrument_config = self.instrument_config.read();
        let resonance_model = instrument_config.resonance_model();
        let excitement = Self::compute_excitement_frame(
            self.analyzer_excitement_feed.pop().unwrap_or_default(),
            feedback_input,
            control_input,
            &tuner_config,
            &instrument_config,
            resonance_model,
        );
        self.update_summary_excitement_snapshot(&excitement);
        Self::write_interleaved_output(&excitement, output);
    }

    fn process(
        &mut self,
        size: usize,
        input: &fundsp::prelude::BufferRef,
        output: &mut fundsp::prelude::BufferMut,
    ) {
        const SIMD_LANES: usize = 8;

        let tuner_config = self.tuner_config.read();
        let instrument_config = self.instrument_config.read();
        let sensor_count = tuner_config.sensor_data.len();
        let feedback_offset = matches!(self.src, ExcitementSource::Mic) as usize;
        let control_offset = feedback_offset + sensor_count;
        let control_channels = if matches!(self.src, ExcitementSource::Manual) {
            sensor_count * <super::node::NodeController<S> as AudioNode>::Inputs::USIZE
        } else {
            0
        };
        let resonance_model = instrument_config.resonance_model();

        let mut feedback_frame = vec![0.0_f32; sensor_count];
        let mut control_frame = vec![0.0_f32; control_channels];

        let simd_frames = size / SIMD_LANES;
        for frame in 0..simd_frames {
            let frame_start = frame * SIMD_LANES;
            for lane in 0..SIMD_LANES {
                let sample = frame_start + lane;

                self.push_input_feed(input, sample);

                for (channel, value) in feedback_frame.iter_mut().enumerate() {
                    *value = input.at_f32(feedback_offset + channel, sample);
                }
                for (channel, value) in control_frame.iter_mut().enumerate() {
                    *value = input.at_f32(control_offset + channel, sample);
                }

                let excitement = Self::compute_excitement_frame(
                    self.analyzer_excitement_feed.pop().unwrap_or_default(),
                    &feedback_frame,
                    &control_frame,
                    &tuner_config,
                    &instrument_config,
                    resonance_model,
                );
                self.update_summary_excitement_snapshot(&excitement);
                Self::write_buffer_sample_output(&excitement, output, sample);
            }
        }

        // Process remainder samples using tick()
        let remainder_start = simd_frames * SIMD_LANES;
        if remainder_start < size {
            drop(tuner_config);
            drop(instrument_config);
            for sample in remainder_start..size {
                let mut input_frame = vec![0.0_f32; self.inputs()];
                for (i, val) in input_frame.iter_mut().enumerate() {
                    *val = input.at_f32(i, sample);
                }
                let mut output_frame = vec![0.0_f32; self.outputs()];
                self.tick(&input_frame, &mut output_frame);
                for (i, val) in output_frame.iter().enumerate() {
                    output.set_f32(i, sample, *val);
                }
            }
        }
    }

    fn inputs(&self) -> usize {
        let feedback_inputs = self.tuner_config.read().sensor_data.len()
            * <super::feedback_pass::FeedbackCatch as AudioNode>::Outputs::USIZE;

        (match self.src {
            ExcitementSource::Mic => 1,
            ExcitementSource::Manual => self.outputs(),
            _ => 0,
        }) + feedback_inputs
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

pub fn create_excitor<S: Float + Real + ordered_float::Float + FloatCore + 'static>(
    src: ExcitementSource,
    tuner_cfg: TunerConfig,
    instrument_cfg: InstrumentConfig,
) -> Excitor<S> {
    Excitor::new(src, tuner_cfg, instrument_cfg, Rng::new())
}

pub fn create_seeded_excitor<S: Float + Real + ordered_float::Float + FloatCore + 'static>(
    src: ExcitementSource,
    tuner_cfg: TunerConfig,
    instrument_cfg: InstrumentConfig,
    seed: u64,
) -> Excitor<S> {
    Excitor::new(src, tuner_cfg, instrument_cfg, Rng::with_seed(seed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::instrument::layout::layout_test_cases;
    use common::tuner::SensorData;
    use insta_fun::prelude::*;
    use std::time::Duration;

    fn test_configs() -> (TunerConfig, InstrumentConfig) {
        let layout = layout_test_cases()
            .next()
            .expect("layout test case must exist");
        let instrument_config =
            InstrumentConfig::try_from(layout).expect("instrument config must be valid");
        let sensor_data = instrument_config
            .0
            .iter()
            .flat_map(|band| band.nodes.iter())
            .map(|node| {
                let center = node.frequency as f32;
                SensorData {
                    key: node.key,
                    min_frequency: (center * 0.95).max(1.0),
                    max_frequency: (center * 1.05).max(1.0 + f32::EPSILON),
                    min_magnitude: 0.0,
                    max_magnitude: 1.0,
                }
            })
            .collect::<Vec<_>>();
        let tuner_config = TunerConfig {
            sensor_data,
            sample_rate: 44_100.0,
            fft_size: FFT_WINDOW_SIZE,
            ..TunerConfig::default()
        };

        (tuner_config, instrument_config)
    }

    fn snapshot_config(
        processing_mode: Processing,
        warm_up: WarmUp,
        num_samples: usize,
        allow_abnormal_samples: bool,
    ) -> SnapshotConfig {
        SnapshotConfigBuilder::default()
            .sample_rate(44_100.0)
            .num_samples(num_samples)
            .with_inputs(true)
            .warm_up(warm_up)
            .processing_mode(processing_mode)
            .allow_abnormal_samples(allow_abnormal_samples)
            .show_grid(true)
            .svg_width(1024)
            .svg_height_per_channel(512)
            .chart_layout(Layout::CombinedPerChannelType)
            .build()
            .expect("snapshot config must be valid")
    }

    /// Stacks `channels` sine oscillators at 110, 220, 330 … Hz into a single multi-output
    /// AudioUnit, initialized and ready to wrap in InputSource::AudioUnit.
    fn sine_driver(channels: usize, sample_rate: f64) -> Box<dyn AudioUnit> {
        let mut net = Net::new(0, channels);
        for ch in 0..channels {
            let node = net.push(Box::new(sine_hz::<f32>(110.0 * (ch as f32 + 1.0))));
            net.connect_output(node, 0, ch);
        }
        let mut unit: Box<dyn AudioUnit> = Box::new(net);
        unit.set_sample_rate(sample_rate);
        unit.reset();
        unit
    }

    fn assert_excitor_snapshot(name: &str, src: ExcitementSource, processing_mode: Processing) {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("tokio runtime must be created");
        let _guard = runtime.enter();

        let (tuner_config, instrument_config) = test_configs();
        let sample_rate_hz = tuner_config.sample_rate;
        let unit = create_seeded_excitor::<f32>(src, tuner_config, instrument_config, 42);
        let input = InputSource::AudioUnit(sine_driver(unit.inputs(), sample_rate_hz as f64));

        let snapshot_samples = FFT_WINDOW_SIZE * 4;

        assert_audio_unit_snapshot!(
            name,
            unit,
            input,
            snapshot_config(processing_mode, WarmUp::None, snapshot_samples, false)
        );
    }

    fn assert_excitor_raw_data(src: ExcitementSource, processing_mode: Processing) {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("tokio runtime must be created");
        let _guard = runtime.enter();

        let (tuner_config, instrument_config) = test_configs();
        let sample_rate_hz = tuner_config.sample_rate;
        let snapshot_samples = FFT_WINDOW_SIZE * 4;
        let unit = create_seeded_excitor::<f32>(src, tuner_config, instrument_config, 42);
        let expected_outputs = unit.outputs();
        let expected_inputs = unit.inputs();

        assert_audio_unit_data!(
            unit,
            InputSource::AudioUnit(sine_driver(expected_inputs, sample_rate_hz as f64)),
            snapshot_config(processing_mode, WarmUp::None, snapshot_samples, true) =>
                |data: &AudioUnitSnapshotData| {
                    assert_eq!(
                        data.output_data.len(),
                        expected_outputs,
                        "unexpected output channel count"
                    );
                    assert_eq!(
                        data.num_samples,
                        snapshot_samples,
                        "unexpected sample count"
                    );

                    let mut max_range = 0.0_f32;
                    for (channel, samples) in data.output_data.iter().enumerate() {
                        assert_eq!(
                            samples.len(),
                            snapshot_samples,
                            "channel {channel} sample count mismatch"
                        );

                        let mut min = f32::INFINITY;
                        let mut max = f32::NEG_INFINITY;
                        for &value in samples {
                            assert!(value.is_finite(), "non-finite sample found in channel {channel}");
                            assert!(
                                value.abs() <= 2.0,
                                "sample amplitude out of expected bounds in channel {channel}: {value}"
                            );
                            min = min.min(value);
                            max = max.max(value);
                        }
                        max_range = max_range.max(max - min);
                    }

                    assert!(
                        max_range > 1.0e-4,
                        "output range should show non-trivial variation"
                    );
                }
        );
    }

    #[test]
    fn excitor_mic_tick_data() {
        assert_excitor_raw_data(ExcitementSource::Mic, Processing::Tick);
    }

    #[test]
    fn excitor_mic_batch_data() {
        assert_excitor_raw_data(ExcitementSource::Mic, Processing::Batch(64));
    }

    #[test]
    fn excitor_entropy_tick_data() {
        assert_excitor_raw_data(ExcitementSource::Entropy, Processing::Tick);
    }

    #[test]
    fn excitor_entropy_batch_data() {
        assert_excitor_raw_data(ExcitementSource::Entropy, Processing::Batch(64));
    }

    #[test]
    fn excitor_manual_tick_snapshot() {
        assert_excitor_snapshot(
            "excitor_manual_tick",
            ExcitementSource::Manual,
            Processing::Tick,
        );
    }

    #[test]
    fn excitor_manual_batch_snapshot() {
        assert_excitor_snapshot(
            "excitor_manual_batch",
            ExcitementSource::Manual,
            Processing::Batch(64),
        );
    }

    #[test]
    fn excitor_summary_snapshot_updates() {
        let (tuner_config, instrument_config) = test_configs();
        let mut unit = create_seeded_excitor::<f32>(
            ExcitementSource::Manual,
            tuner_config,
            instrument_config,
            42,
        );

        let input = vec![0.0_f32; unit.inputs()];
        let mut output = vec![0.0_f32; unit.outputs()];
        unit.tick(&input, &mut output);

        let snapshot = unit.summary_snapshot();
        let summary = snapshot.read();
        assert_eq!(summary.len(), unit.outputs() / 2);
    }

    #[test]
    fn excitor_spectrum_feed_populated_by_analyzer() {
        let (tuner_config, instrument_config) = test_configs();
        let mut unit = create_seeded_excitor::<f32>(
            ExcitementSource::Mic,
            tuner_config,
            instrument_config,
            1337,
        );

        let input_len = unit.inputs();
        let output_len = unit.outputs();
        let mut driver = sine_driver(input_len, 44_100.0);
        let mut input = vec![0.0_f32; input_len];
        let mut output = vec![0.0_f32; output_len];

        for _ in 0..(FFT_WINDOW_SIZE + 256) {
            driver.tick(&[], &mut input);
            unit.tick(&input, &mut output);
        }

        let spectrum_feed = unit.spectrum_feed();
        let mut spectrum = spectrum_feed.pop();
        if spectrum.is_none() {
            for _ in 0..20 {
                std::thread::sleep(Duration::from_millis(5));
                spectrum = spectrum_feed.pop();
                if spectrum.is_some() {
                    break;
                }
            }
        }

        assert!(
            spectrum.is_some(),
            "expected analyzer to publish at least one spectrum snapshot"
        );
    }
}
