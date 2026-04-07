use num_rational::Rational32;

pub struct Grid {
    sample_rate: f64,
    bpm: u32,
}

pub struct Cadastre {
    /// Grid position of the first tick. E.g. if the first tick is on the 3rd beat, this would be 3/4.
    pub start: Rational32,
    /// Segment length in grid units. E.g. if the segment is 1 beat long, this would be 1/1 or 16/16 etc.
    pub duration: Rational32,
}
