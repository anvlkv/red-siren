use std::sync::atomic::AtomicBool;

use fundsp::hacker32::Snoop;
use hecs::Entity;

use crate::buf::AppAuBuffer;
use crate::system::System;
use ::shared::UnitState;

#[derive(Default)]
pub struct UnitModel {
    pub sample_rate: u32,
    pub fft_res: usize,
    pub buffer_size: u32,
    pub state: UnitState,
    pub app_au_buffer: AppAuBuffer,
    pub input_analyzer_enabled: AtomicBool,
    pub system: Option<System>,
    pub snoops: Vec<(Snoop<f32>, Entity)>,
}
