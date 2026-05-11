mod band;
mod channel;
mod context;
mod layout;
mod node;
mod orientation;
mod scale;

pub use band::*;
pub use channel::*;
pub use context::*;
pub use layout::*;
pub use node::*;
pub use orientation::*;
pub use scale::*;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct Config {
    pub ctx_hash: u64,
    pub ctx: Context,
    pub layout: Layout,
    pub scale: Scale,
    pub base_physics: NodePhysics,
    pub base_band: Band,
}
