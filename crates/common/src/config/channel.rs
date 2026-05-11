use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// Output channel of the band
pub enum BandChannel {
    Left,
    Right,
}

impl BandChannel {
    pub fn nth_channel_from_first(&self, n: usize) -> Self {
        if n.is_multiple_of(2) {
            *self
        } else {
            match self {
                Self::Left => Self::Right,
                Self::Right => Self::Left,
            }
        }
    }
}
