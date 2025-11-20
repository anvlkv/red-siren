#[cfg(feature = "hi_fi")]
use fundsp::hacker::prelude::*;
#[cfg(not(feature = "hi_fi"))]
use fundsp::hacker32::prelude::*;

use crate::util::hash_str;

const DB_LIN_CONVERTER_ID: u64 = hash_str(concat!(module_path!(), "::DbLinConverter"));

#[derive(Clone, Copy)]
pub struct DbLinConverter;

pub fn db_lin_converter() -> An<DbLinConverter> {
    An(DbLinConverter)
}

impl AudioNode for DbLinConverter {
    const ID: u64 = DB_LIN_CONVERTER_ID;

    type Inputs = U1;

    type Outputs = U1;

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        [db_amp(input[0])].into()
    }
}
