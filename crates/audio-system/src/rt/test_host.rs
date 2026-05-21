//! Minimal CPAL test host for deterministic engine testing (no real audio I/O).
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{
    DeviceDescription, DeviceDescriptionBuilder, DeviceId, HostId, SampleFormat,
    SupportedBufferSize, SupportedStreamConfig, SupportedStreamConfigRange,
};
use std::time::Duration;

#[derive(Clone)]
pub struct TestDevice;

impl DeviceTrait for TestDevice {
    type SupportedInputConfigs = std::iter::Once<SupportedStreamConfigRange>;
    type SupportedOutputConfigs = std::iter::Once<SupportedStreamConfigRange>;
    type Stream = TestStream;

    fn description(&self) -> Result<DeviceDescription, cpal::DeviceNameError> {
        Ok(DeviceDescriptionBuilder::new("Test Device".to_string()).build())
    }
    fn id(&self) -> Result<DeviceId, cpal::DeviceIdError> {
        Ok(DeviceId(HostId::Custom, "test-device".to_string()))
    }
    fn supports_input(&self) -> bool {
        true
    }
    fn supports_output(&self) -> bool {
        true
    }
    fn supported_input_configs(
        &self,
    ) -> Result<Self::SupportedInputConfigs, cpal::SupportedStreamConfigsError> {
        Ok(std::iter::once(SupportedStreamConfigRange::new(
            1,
            44_100,
            44_100,
            SupportedBufferSize::Unknown,
            SampleFormat::F32,
        )))
    }
    fn supported_output_configs(
        &self,
    ) -> Result<Self::SupportedOutputConfigs, cpal::SupportedStreamConfigsError> {
        Ok(std::iter::once(SupportedStreamConfigRange::new(
            1,
            44_100,
            44_100,
            SupportedBufferSize::Unknown,
            SampleFormat::F32,
        )))
    }
    fn default_input_config(
        &self,
    ) -> Result<SupportedStreamConfig, cpal::DefaultStreamConfigError> {
        Ok(SupportedStreamConfig::new(
            1,
            44_100,
            SupportedBufferSize::Unknown,
            SampleFormat::F32,
        ))
    }
    fn default_output_config(
        &self,
    ) -> Result<SupportedStreamConfig, cpal::DefaultStreamConfigError> {
        self.default_input_config()
    }
    fn build_input_stream_raw<D, E>(
        &self,
        _config: &cpal::StreamConfig,
        _sample_format: SampleFormat,
        _data_callback: D,
        _error_callback: E,
        _timeout: Option<Duration>,
    ) -> Result<Self::Stream, cpal::BuildStreamError>
    where
        D: FnMut(&cpal::Data, &cpal::InputCallbackInfo) + Send + 'static,
        E: FnMut(cpal::StreamError) + Send + 'static,
    {
        Ok(TestStream)
    }
    fn build_output_stream_raw<D, E>(
        &self,
        _config: &cpal::StreamConfig,
        _sample_format: SampleFormat,
        _data_callback: D,
        _error_callback: E,
        _timeout: Option<Duration>,
    ) -> Result<Self::Stream, cpal::BuildStreamError>
    where
        D: FnMut(&mut cpal::Data, &cpal::OutputCallbackInfo) + Send + 'static,
        E: FnMut(cpal::StreamError) + Send + 'static,
    {
        Ok(TestStream)
    }
}

pub struct TestStream;
impl StreamTrait for TestStream {
    fn play(&self) -> Result<(), cpal::PlayStreamError> {
        Ok(())
    }
    fn pause(&self) -> Result<(), cpal::PauseStreamError> {
        Ok(())
    }
}

pub struct TestHost;
impl HostTrait for TestHost {
    type Devices = std::iter::Once<TestDevice>;
    type Device = TestDevice;

    fn is_available() -> bool {
        true
    }
    fn devices(&self) -> Result<Self::Devices, cpal::DevicesError> {
        Ok(std::iter::once(TestDevice))
    }
    fn default_input_device(&self) -> Option<Self::Device> {
        Some(TestDevice)
    }
    fn default_output_device(&self) -> Option<Self::Device> {
        Some(TestDevice)
    }
}

// Helper to construct a CPAL Host from TestHost
pub fn make_test_host() -> cpal::Host {
    cpal::platform::CustomHost::from_host(TestHost).into()
}
