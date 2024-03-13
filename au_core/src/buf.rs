use std::sync::Arc;

use parking_lot::Mutex;
use ringbuf::{Rb, StaticRb};

use crate::{FFTData, SnoopsData};

pub const FFT_BUF_SIZE: usize = 4;
pub const SNOOPS_BUF_SIZE: usize = 12;

pub type FFTBuf = StaticRb<FFTData, FFT_BUF_SIZE>;
pub type SnoopsBuf = StaticRb<SnoopsData, SNOOPS_BUF_SIZE>;

#[derive(Default, Clone)]
pub struct AppAuBuffer {
    fft_rb: Arc<Mutex<FFTBuf>>,
    snoops_rb: Arc<Mutex<SnoopsBuf>>,
}

impl AppAuBuffer {
    pub fn push_fft_data(&self, data: FFTData) {
        let mut buf = self.fft_rb.lock();
        _ = buf.push_overwrite(data);
    }

    pub fn push_snoops_data(&self, data: SnoopsData) {
        let mut buf = self.snoops_rb.lock();
        _ = buf.push_overwrite(data);
    }

    pub fn read_fft_data(&self) -> Option<FFTData> {
        let mut buf = self.fft_rb.try_lock();
        let buf = buf.as_mut();
        buf.map(|b| b.pop()).flatten()
    }

    pub fn read_snoops_data(&self) -> Option<SnoopsData> {
        let mut buf = self.snoops_rb.try_lock();
        let buf = buf.as_mut();
        buf.map(|b| b.pop()).flatten()
    }
}
