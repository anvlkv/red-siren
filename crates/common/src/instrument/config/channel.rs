use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// Output chanel of the group
pub enum GroupChannel {
    Left,
    Right,
}

impl GroupChannel {
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
