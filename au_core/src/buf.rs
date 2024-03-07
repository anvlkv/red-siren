use std::sync::Arc;

use once_cell::sync::Lazy;
use ringbuf::{Consumer, Producer, StaticRb};

use crate::{FFTData, SnoopsData, FFT_BUF_SIZE, SNOOPS_BUF_SIZE};

pub type FFTBuf = StaticRb<FFTData, FFT_BUF_SIZE>;
pub type SnoopsBuf = StaticRb<SnoopsData, SNOOPS_BUF_SIZE>;

pub type FFTProd = Producer<FFTData, Arc<FFTBuf>>;
pub type FFTCons = Consumer<FFTData, Arc<FFTBuf>>;
pub type SnoopsProd = Producer<SnoopsData, Arc<SnoopsBuf>>;
pub type SnoopsCons = Consumer<SnoopsData, Arc<SnoopsBuf>>;

static mut FFT_RB: Lazy<(FFTProd, FFTCons)> = Lazy::new(|| StaticRb::default().split());

static mut SNOOPS_RB: Lazy<(SnoopsProd, SnoopsCons)> = Lazy::new(|| StaticRb::default().split());

pub fn fft_prod() -> &'static mut FFTProd {
    unsafe { &mut FFT_RB.0 }
}

pub fn fft_cons() -> &'static mut FFTCons {
    unsafe { &mut FFT_RB.1 }
}

pub fn snoops_prod() -> &'static mut SnoopsProd {
    unsafe { &mut SNOOPS_RB.0 }
}

pub fn snoops_cons() -> &'static mut SnoopsCons {
    unsafe { &mut SNOOPS_RB.1 }
}
