use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// Output chanel of the group
pub enum GroupChannel {
    Left,
    Right,
}

impl GroupChannel {
    pub(crate) fn from_keys_groups(k: u32, g: u32) -> Self {
        match (g.is_multiple_of(2), k.is_multiple_of(2)) {
            (false, false) => Self::Left,
            (false, true) => Self::Right,
            (true, false) => Self::Left,
            (true, true) => Self::Right,
        }
    }

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
