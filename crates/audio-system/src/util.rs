#[cfg(not(feature = "hi_fi"))]
use fundsp::math::Complex32;
#[cfg(feature = "hi_fi")]
use fundsp::math::Complex64;

pub const fn hash_str(s: &str) -> u64 {
    // FNV-1a hash algorithm (const-friendly)
    let mut hash = 0xcbf29ce484222325u64;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(0x100000001b3);
        i += 1;
    }
    hash
}

#[cfg(not(feature = "hi_fi"))]
pub type S = f32;
#[cfg(not(feature = "hi_fi"))]
pub type SComplex = Complex32;

#[cfg(feature = "hi_fi")]
pub type S = f64;
#[cfg(feature = "hi_fi")]
pub type SComplex = Complex64;

pub type DbLin = fundsp::hacker::Pipe<
    crate::values::FineTunedValue,
    super::system::output::db_lin::DbLinConverter,
>;
