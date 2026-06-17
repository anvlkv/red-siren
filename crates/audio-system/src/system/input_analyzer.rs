use std::{
    sync::{atomic::AtomicBool, Arc},
    thread::JoinHandle,
};

use fundsp::{fft::real_fft, prelude::*, thingbuf::ThingBuf};
use parking_lot::RwLock;

#[derive(Clone)]
pub struct InputAnalyzer {
    analyzer_job: Option<Arc<JoinHandle<()>>>,
    job_running: Arc<AtomicBool>,
    input_buffer: Arc<ThingBuf<f32>>,
    spectrum_thb: Arc<ThingBuf<Vec<Complex32>>>,
    window_size: Arc<RwLock<usize>>,
    overlap: Arc<RwLock<f64>>,
}

impl InputAnalyzer {
    pub fn new(initial_window_size: usize, initial_overlap: f64) -> Self {
        let input_buffer = Arc::new(ThingBuf::<f32>::new(initial_window_size));
        let spectrum_thb = Arc::new(ThingBuf::<Vec<Complex32>>::new(initial_window_size / 2 + 1));
        let window_size = Arc::new(RwLock::new(initial_window_size));
        let overlap = Arc::new(RwLock::new(initial_overlap.clamp(0.0, 1.0)));
        let job_running = Arc::new(AtomicBool::new(true));

        let analyzer_job = Some(Arc::new({
            let input_buffer = input_buffer.clone();
            let spectrum_thb = spectrum_thb.clone();
            let window_size = window_size.clone();
            let overlap = overlap.clone();
            let job_running = job_running.clone();

            std::thread::spawn(move || {
                let mut local_input_buffer: Vec<f32> = Vec::new();
                let mut oversampled_buffer: Vec<f32> = Vec::new();

                loop {
                    if !job_running.load(std::sync::atomic::Ordering::SeqCst) {
                        break;
                    }
                    let window_size = *window_size.read();
                    let overlap_size = (*overlap.read() * window_size as f64) as usize;

                    if oversampled_buffer.len() != window_size {
                        oversampled_buffer.resize(window_size, 0.0);
                    }

                    // Read from the input buffer and fill the local buffer
                    while let Some(sample) = input_buffer.pop() {
                        local_input_buffer.push(sample);
                    }

                    if local_input_buffer.len() >= overlap_size {
                        _ = spectrum_thb
                            .push_with(|spectrum| {
                                // Shift the oversampled buffer to the left by the overlap size
                                oversampled_buffer.copy_within(overlap_size.., 0);
                                // Fill the end of the oversampled buffer with new samples from the local buffer
                                let new_samples = &local_input_buffer[..window_size - overlap_size];
                                oversampled_buffer[window_size - overlap_size..]
                                    .copy_from_slice(new_samples);
                                // Remove the used samples from the local buffer
                                local_input_buffer.drain(..window_size - overlap_size);

                                // Perform FFT on the oversampled buffer and store the result in the spectrum
                                let fft_result = real_fft(&mut oversampled_buffer);
                                spectrum.copy_from_slice(fft_result);
                            })
                            .ok();
                    }
                }
            })
        }));

        Self {
            analyzer_job,
            input_buffer,
            spectrum_thb,
            window_size,
            overlap,
            job_running,
        }
    }

    pub fn get_spectrum(&self) -> Option<Vec<Complex32>> {
        self.spectrum_thb.pop()
    }

    pub fn set_window_size(&self, size: usize) {
        *self.window_size.write() = size;
    }

    pub fn set_overlap(&self, overlap: f64) {
        *self.overlap.write() = overlap.clamp(0.0, 1.0);
    }
}

impl Drop for InputAnalyzer {
    fn drop(&mut self) {
        if let Some(job) = self
            .analyzer_job
            .take()
            .and_then(|a| Arc::try_unwrap(a).ok())
        {
            self.job_running
                .store(false, std::sync::atomic::Ordering::SeqCst);
            if let Err(e) = job.join() {
                log::error!("Failed to join analyzer job: {:?}", e);
            }
        }
    }
}

const INPUT_ANALYZER_ID: u64 = crate::util::hash_str(concat!(module_path!(), "::InputAnalyzer"));

impl AudioNode for InputAnalyzer {
    const ID: u64 = INPUT_ANALYZER_ID;

    type Inputs = U1;

    type Outputs = U0;

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        // Write the input to the input buffer for analysis
        if let Err(full) = self.input_buffer.push(input[0]) {
            _ = self.input_buffer.pop(); // Remove the oldest sample to make room
            self.input_buffer.push(full.into_inner()).unwrap();
        }
        Frame::default()
    }
}
