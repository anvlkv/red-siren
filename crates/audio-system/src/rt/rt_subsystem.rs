use std::{collections::BTreeMap, sync::Arc, thread, time::Duration};

use common::error::{AudioAnalysisError, Result};
use fundsp::prelude::*;
use parking_lot::RwLock;

use crate::{
    system::{mixer::create_mixer, output_analyzer},
    SampleType,
};

pub const FADE_DURATION_MS: u64 = 120;
const FOLLOW_RESPONSE_SECS: f32 = FADE_DURATION_MS as f32 / 1000.0;

pub struct RuntimeSubsystem {
    /// DSP network (frontend)
    dsp_network: Arc<RwLock<Net>>,
    /// Configurable DSP network
    inner_network_id: NodeId,
    /// Configurable output subnet
    output_subnet_id: NodeId,
    /// Sample type
    sample_type: SampleType,
    /// Number of  output channels
    num_channels: usize,
    /// Output gain parameter
    gain_param: Arc<Shared>,
    /// Snoop buffers for processed output
    processed_output_snoops: Arc<RwLock<(Snoop, Snoop)>>,
    /// Samples buffer for output spectrum analysis
    spectrum_buffer: Arc<RwLock<[[f32; output_analyzer::OUTPUT_ANALYZER_FFT_WINDOW_SIZE]; 2]>>,
    /// Snoop buffer for raw input
    raw_input_snoops: Arc<RwLock<Snoop>>,
}

impl RuntimeSubsystem {
    fn reset_analysis_state(&self) {
        {
            let mut snoops = self.processed_output_snoops.write();
            let _ = snoops.0.get();
            let _ = snoops.1.get();
        }

        {
            let mut raw_input = self.raw_input_snoops.write();
            let _ = raw_input.get();
        }

        {
            let mut spectrum_buffer = self.spectrum_buffer.write();
            spectrum_buffer[0].fill(0.0);
            spectrum_buffer[1].fill(0.0);
        }
    }

    pub fn new(sample_type: SampleType, num_channels: usize, inner_net: Option<Net>) -> Self {
        let spectrum_buffer = Arc::new(RwLock::new(
            [[0_f32; output_analyzer::OUTPUT_ANALYZER_FFT_WINDOW_SIZE]; 2],
        ));

        let mut dsp_network = Net::new(1, num_channels);

        let gain_param = Arc::new(Shared::new(0.0));

        let inner_network_id = dsp_network.push(match inner_net {
            Some(net) => Box::new(net),
            None => Box::new(sink() | constant::<Frame<f32, U2>>(Frame::default())),
        });

        let (output_net, (l_snoop, r_snoop)) =
            Self::create_output_node(sample_type, num_channels, gain_param.clone());

        let output_subnet_id = dsp_network.push(output_net);

        let (in_snoop, in_snoop_be) = snoop(240);
        let input_subnet_id = dsp_network.push(Box::new(in_snoop_be));

        dsp_network.pipe_input(input_subnet_id);
        dsp_network.pipe_all(input_subnet_id, inner_network_id);
        dsp_network.pipe_all(inner_network_id, output_subnet_id);
        dsp_network.pipe_output(output_subnet_id);

        dsp_network.check();

        Self {
            dsp_network: Arc::new(RwLock::new(dsp_network)),
            inner_network_id,
            output_subnet_id,
            sample_type,
            num_channels,
            gain_param,
            spectrum_buffer,
            processed_output_snoops: Arc::new(RwLock::new((l_snoop, r_snoop))),
            raw_input_snoops: Arc::new(RwLock::new(in_snoop)),
        }
    }

    pub fn backend(&self) -> NetBackend {
        self.dsp_network.write().backend()
    }

    pub fn fade_out(&self) {
        self.gain_param.set_value(0.0);
        thread::sleep(Duration::from_millis(FADE_DURATION_MS));
    }

    pub fn fade_in(&self) {
        self.gain_param.set_value(1.0);
        thread::sleep(Duration::from_millis(FADE_DURATION_MS));
    }

    pub fn set_sample_rate(&self, sample_rate: f64) {
        let mut dsp_network = self.dsp_network.write();
        dsp_network.set_sample_rate(sample_rate);
        if dsp_network.has_backend() {
            dsp_network.commit();
        }
    }

    pub fn set_sample_type(&mut self, sample_type: SampleType) {
        let (output_net, (l_snoop, r_snoop)) =
            Self::create_output_node(sample_type, self.num_channels, self.gain_param.clone());

        {
            let mut dsp_network = self.dsp_network.write();
            dsp_network.replace(self.output_subnet_id, output_net);

            if dsp_network.has_backend() {
                dsp_network.commit();
            }
        }
        *self.processed_output_snoops.write() = (l_snoop, r_snoop);
        self.reset_analysis_state();
        self.sample_type = sample_type;
    }

    pub fn set_num_channels(&mut self, num_channels: usize) {
        let (output_net, (l_snoop, r_snoop)) =
            Self::create_output_node(self.sample_type, num_channels, self.gain_param.clone());

        {
            let mut dsp_network = self.dsp_network.write();
            dsp_network.replace(self.output_subnet_id, output_net);

            if dsp_network.has_backend() {
                dsp_network.commit();
            }
        }
        *self.processed_output_snoops.write() = (l_snoop, r_snoop);
        self.reset_analysis_state();
        self.num_channels = num_channels;
    }

    fn create_output_node(
        sample_type: SampleType,
        num_channels: usize,
        gain: Arc<Shared>,
    ) -> (Box<dyn AudioUnit>, (Snoop, Snoop)) {
        let (l_snoop, l_snoop_be) = snoop(output_analyzer::OUTPUT_ANALYZER_FFT_WINDOW_SIZE);
        let (r_snoop, r_snoop_be) = snoop(output_analyzer::OUTPUT_ANALYZER_FFT_WINDOW_SIZE);
        let gain = var(&gain) >> follow(FOLLOW_RESPONSE_SECS) >> split::<U2>();
        let stereo_snoop_gain = (l_snoop_be | r_snoop_be) * gain;

        let node = u_num_it::u_num_it!(
            1..=8,
            match num_channels {
                U => {
                    match sample_type {
                        SampleType::F32 => {
                            Box::new(stereo_snoop_gain >> create_mixer::<f32, U2, NumType>())
                                as Box<dyn AudioUnit>
                        }
                        SampleType::F64 => {
                            Box::new(stereo_snoop_gain >> create_mixer::<f64, U2, NumType>())
                                as Box<dyn AudioUnit>
                        }
                    }
                }
            }
        );

        (node, (l_snoop, r_snoop))
    }

    pub fn set_inner_network(&self, new_net: Net) {
        self.fade_out();
        {
            let mut dsp_network = self.dsp_network.write();
            dsp_network.replace(self.inner_network_id, Box::new(new_net));

            if dsp_network.has_backend() {
                dsp_network.commit();
            }
        }
        self.reset_analysis_state();
        self.fade_in();
    }

    pub fn output_spectrum(
        &self,
        sample_rate: f64,
        min: f32,
        max: f32,
    ) -> Result<(BTreeMap<u32, f32>, BTreeMap<u32, f32>)> {
        let (l_data, r_data) = {
            let mut snoops = self.processed_output_snoops.write();

            (
                snoops
                    .0
                    .get()
                    .into_iter()
                    .flat_map(|buff| (0..buff.len()).map(|i| buff.at(i)).collect::<Vec<_>>())
                    .collect::<Vec<f32>>(),
                snoops
                    .1
                    .get()
                    .into_iter()
                    .flat_map(|buff| (0..buff.len()).map(|i| buff.at(i)).collect::<Vec<_>>())
                    .collect::<Vec<f32>>(),
            )
        };

        if l_data.len() == 0 || r_data.len() == 0 {
            return Err(AudioAnalysisError::EmptyOutputBuffer.into());
        }

        let (l_window, r_window) = {
            let mut spectrum_buffer = self.spectrum_buffer.write();

            let start_l = spectrum_buffer[0].len() - l_data.len();
            let start_r = spectrum_buffer[1].len() - r_data.len();

            spectrum_buffer[0].rotate_left(l_data.len());
            spectrum_buffer[1].rotate_left(r_data.len());

            spectrum_buffer[0][start_l..].copy_from_slice(&l_data);
            spectrum_buffer[1][start_r..].copy_from_slice(&r_data);
            (spectrum_buffer[0], spectrum_buffer[1])
        };

        let l_analysis = output_analyzer::analyze(l_window, sample_rate, min, max)?;
        let r_analysis = output_analyzer::analyze(r_window, sample_rate, min, max)?;

        Ok((l_analysis, r_analysis))
    }

    pub fn input_snapshot(&self) -> Vec<f32> {
        let mut in_snoop = self.raw_input_snoops.write();
        in_snoop
            .get()
            .into_iter()
            .flat_map(|buff| (0..buff.len()).map(|i| buff.at(i)).collect::<Vec<_>>())
            .collect::<Vec<f32>>()
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeSubsystem;
    use crate::SampleType;

    #[test]
    fn new_and_basic_accessors_are_callable() {
        let rt = RuntimeSubsystem::new(SampleType::F32, 2, None);

        rt.set_sample_rate(44_100.0);
        let input = rt.input_snapshot();
        assert!(input.len() <= 240);
    }

    #[test]
    fn reconfiguration_paths_are_callable() {
        let mut rt = RuntimeSubsystem::new(SampleType::F32, 2, None);

        rt.set_sample_type(SampleType::F64);
        rt.set_num_channels(2);
        rt.set_sample_rate(48_000.0);
    }

    #[test]
    fn output_spectrum_is_callable_for_valid_range() {
        let rt = RuntimeSubsystem::new(SampleType::F32, 2, None);

        let spectrum = rt.output_spectrum(44_100.0, 20.0, 20_000.0);
        assert!(spectrum.is_ok());
    }
}
