#[cfg(feature = "egui")]
pub mod egui_testbed;
pub mod quality;
pub mod rt;
pub mod system;
pub mod test_support;
pub mod util;
pub use quality::{PlaybackQualityGate, SampleType};
pub use rt::telemetry::{create_telemetry_channel, TelemetrySender};

pub const FFT_WINDOW_SIZE: usize = 512;
