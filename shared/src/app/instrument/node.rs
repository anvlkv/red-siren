use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct Node {
    pub base_freq: f64,
    pub max_freq: f64,
    pub f_n: usize,
}

impl Eq for Node {}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.base_freq.partial_cmp(&other.base_freq) {
            Some(core::cmp::Ordering::Equal) => {}
            ord => return ord,
        }
        self.max_freq.partial_cmp(&other.max_freq)
    }
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl Node {
    pub fn new((base_freq, max_freq): (f64, f64), f_n: usize) -> Self {
        Self {
            base_freq,
            max_freq,
            f_n,
        }
    }
}
