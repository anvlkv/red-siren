use crate::create_telemetry_channel;
use crate::rt::engine::Engine;
use crate::rt::rt_subsystem::RuntimeSubsystem;
use crate::SampleType;

use super::test_host::make_test_host;

#[test]
fn engine_starts_with_test_host() {
    let test_host = make_test_host();
    let (telemetry, _rx) = create_telemetry_channel();
    let engine = Engine::new_with_host(telemetry, None, None, Some(test_host));
    // Should be able to list devices and select default devices without error
    let devices = engine.list_devices();
    assert!(
        !devices.is_empty(),
        "Test host should provide at least one device"
    );
    // Try selecting the first device as both input and output
    let dev = &devices[0];
    assert!(engine
        .select_input_device(dev.host_id.clone(), dev.device_id.clone())
        .is_ok());
    assert!(engine
        .select_output_device(dev.host_id.clone(), dev.device_id.clone())
        .is_ok());
}

#[test]
fn rt_subsystem_public_api_smoke() {
    let mut rt = RuntimeSubsystem::new(SampleType::F32, 2, None);
    rt.set_sample_rate(44_100.0);
    rt.set_sample_type(SampleType::F64);
    rt.set_num_channels(2);

    let spectrum = rt.output_spectrum(44_100.0, 20.0, 20_000.0);
    assert!(spectrum.is_ok());
}
