use serde::{Deserialize, Serialize};

use crate::{safe_area::SafeArea, Rect};

#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub struct Context {
    // Screen
    pub screen_estate: Rect,
    pub dpi: u32,
    pub safe_area: SafeArea,
    pub is_dark_mode: bool,
    // CPU
    pub num_threads: usize,
    pub cpu_frequency_mhz: u64,
    // Audio
    pub sample_rate_hz: u32,
    pub num_channels: u16,
}
