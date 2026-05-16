mod band;
mod channel;
mod context;
mod grid_limits;
mod layout;
mod node;
mod orientation;
mod scale;

pub use band::*;
pub use channel::*;
pub use context::*;
pub use grid_limits::*;
pub use layout::*;
pub use node::*;
pub use orientation::*;
pub use scale::*;

use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Config {
    pub ctx_hash: u64,
    pub ctx: Context,
    pub layout: Layout,
    pub scale: Scale,
    pub bands: Vec<(Band, Vec<Node>)>,
    pub grid_limits: GridLimits,
}

impl Config {
    // pub fn new(ctx: Context) -> Self {
    //     let ctx_hash = Self::hash_context(&ctx);
    //     let layout = Layout::new(&ctx.screen_estate, &ctx.safe_area);
    //     let scale = if ctx.is_dark_mode {
    //         Scale::In
    //     } else {
    //         Scale::Yo
    //     };
    //     let grid_limits = GridLimits::new(
    //         ctx.num_threads,
    //         ctx.cpu_frequency_mhz,
    //         ctx.sample_rate_hz,
    //         ctx.buffer_size_samples,
    //     );
    //     let base_physics = NodePhysics::base(&layout, &scale, &grid_limits);
    //     let base_band = Band::default(&layout, &base_physics);

    //     Self {
    //         ctx_hash,
    //         ctx,
    //         layout,
    //         scale,
    //         base_physics,
    //         base_band,
    //         grid_limits,
    //     }
    // }

    fn hash_context(ctx: &Context) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        ctx.hash(&mut hasher);
        hasher.finish()
    }
}
