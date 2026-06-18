use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::Duration,
};

use fundsp::{fft::real_fft, prelude::*, thingbuf::ThingBuf};
use spectrum_analyzer::windows::hann_window;

#[derive(Clone)]
pub struct InputAnalyzer {
    analyzer_job: Option<Arc<JoinHandle<()>>>,
    job_running: Arc<AtomicBool>,
    input_buffer: Arc<ThingBuf<f32>>,
    spectrum_thb: Arc<ThingBuf<Vec<Complex32>>>,
    window_size: usize,
    overlap: f64,
    hop_size: usize,
}

impl InputAnalyzer {
    pub fn new(initial_window_size: usize, initial_overlap: f64) -> Self {
        const SPECTRUM_QUEUE_CAPACITY: usize = 8;
        let window_size = std::cmp::max(initial_window_size, 2);
        // Keep overlap strictly below 1.0 so hop size always stays non-zero.
        let overlap = initial_overlap.clamp(0.0, 0.999_999);
        let overlap_size = std::cmp::min((overlap * window_size as f64) as usize, window_size - 1);
        let hop_size = std::cmp::max(window_size - overlap_size, 1);

        let input_buffer = Arc::new(ThingBuf::<f32>::new(window_size));
        let spectrum_thb = Arc::new(ThingBuf::<Vec<Complex32>>::new(SPECTRUM_QUEUE_CAPACITY));
        let job_running = Arc::new(AtomicBool::new(true));

        let analyzer_job = Some(Arc::new({
            let input_buffer = input_buffer.clone();
            let spectrum_thb = spectrum_thb.clone();
            let job_running = job_running.clone();

            std::thread::spawn(move || {
                let mut local_input_buffer: Vec<f32> = Vec::new();
                let mut frame_buffer = vec![0.0_f32; window_size];
                let spectrum_len = window_size / 2 + 1;
                let idle_sleep = Duration::from_millis(1);

                loop {
                    if !job_running.load(Ordering::SeqCst) {
                        break;
                    }

                    // Read from the input buffer and fill the local buffer
                    let mut got_samples = false;
                    while let Some(sample) = input_buffer.pop() {
                        local_input_buffer.push(sample);
                        got_samples = true;
                    }

                    let mut produced_frame = false;
                    while local_input_buffer.len() >= hop_size {
                        produced_frame = true;
                        _ = spectrum_thb
                            .push_with(|spectrum| {
                                // Shift by one hop, append fresh samples, then window before FFT.
                                frame_buffer.copy_within(hop_size.., 0);
                                let new_samples = &local_input_buffer[..hop_size];
                                frame_buffer[window_size - hop_size..].copy_from_slice(new_samples);
                                local_input_buffer.drain(..hop_size);

                                let mut windowed = hann_window(&frame_buffer);
                                let fft_result = real_fft(&mut windowed);

                                if spectrum.len() != spectrum_len {
                                    spectrum.resize(spectrum_len, Complex32::default());
                                }
                                spectrum.copy_from_slice(fft_result);
                            })
                            .ok();
                    }

                    if !got_samples && !produced_frame {
                        std::thread::sleep(idle_sleep);
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
            hop_size,
            job_running,
        }
    }

    pub fn get_spectrum(&self) -> Option<Vec<Complex32>> {
        self.spectrum_thb.pop()
    }

    pub fn window_size(&self) -> usize {
        self.window_size
    }

    pub fn overlap(&self) -> f64 {
        self.overlap
    }

    pub fn hop_size(&self) -> usize {
        self.hop_size
    }
}

impl Drop for InputAnalyzer {
    fn drop(&mut self) {
        if let Some(job) = self
            .analyzer_job
            .take()
            .and_then(|a| Arc::try_unwrap(a).ok())
        {
            self.job_running.store(false, Ordering::SeqCst);
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
            if self.input_buffer.push(full.into_inner()).is_err() {
                log::warn!("Input analyzer dropped a sample because the queue remained full");
            }
        }
        Frame::default()
    }
}
