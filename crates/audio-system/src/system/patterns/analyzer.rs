use num_complex::Complex32;

use super::pattern::Pattern;

pub struct RatedPattern {
    pub pattern: Pattern,
    pub rarity: f32,
    pub confidence: f32,
}

pub fn find_patterns(data: &[&[Complex32]]) -> Vec<RatedPattern> {
    let mut patterns: Vec<RatedPattern> = Vec::new();

    patterns
}

pub fn optimize_patterns(patterns: Vec<RatedPattern>, top_n: usize) -> Vec<RatedPattern> {
    patterns
}
