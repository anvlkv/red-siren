use std::sync::{atomic::AtomicBool, Arc};

use fundsp::hacker32::Snoop;
use hecs::Entity;
use shared_types::UnitState;

use crate::{AppAuBuffer, System};

#[derive(Default)]
pub struct UnitModel {
    pub sample_rate: u32,
    pub fft_res: usize,
    pub buffer_size: u32,
    pub system: Option<System>,
    pub state: UnitState,
    pub app_au_buffer: Arc<AppAuBuffer>,
    pub snoops: Vec<(Snoop<f32>, Entity)>,
    pub input_analyzer_enabled: AtomicBool,
}

// #[cfg(not(target_arch = "wasm32"))]
// in_stream: Option<cpal::Stream>,
// #[cfg(not(target_arch = "wasm32"))]
// out_stream: Option<cpal::Stream>,
