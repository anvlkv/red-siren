use fundsp::prelude::*;

use crate::rt::ExcitementSource;

pub mod control;
pub mod manual;
pub mod mic;
pub mod random;

pub fn mount_excitor_au(net: &mut Net, src: ExcitementSource) -> NodeId {
    todo!()
}
