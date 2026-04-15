use fundsp::prelude::*;

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

const DB_LIN_CONVERTER_ID: u64 = hash_str(concat!(module_path!(), "::DbLinConverter"));

/// Converts a dB value to a linear amplitude multiplier.
#[derive(Clone, Copy)]
pub struct DbLinConverter;

impl AudioNode for DbLinConverter {
    const ID: u64 = DB_LIN_CONVERTER_ID;

    type Inputs = U1;
    type Outputs = U1;

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        [db_amp(input[0])].into()
    }
}

pub fn db_lin_converter() -> An<DbLinConverter> {
    An(DbLinConverter)
}

pub type DbLin = fundsp::prelude::Pipe<crate::values::FineTunedValue, DbLinConverter>;
