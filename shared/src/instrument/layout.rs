use std::num::NonZero;

pub enum LayoutOrientation {
    Vertical,
    Horizontal,
}

pub struct Layout {
    pub orientation: LayoutOrientation,
    pub num_keys: NonZero<u8>,
}
