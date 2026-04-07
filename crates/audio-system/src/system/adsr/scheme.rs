#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct Scheme {
    pub atack: u64,
    pub decay: u64,
    pub sustain: u64,
    pub release: u64,
    pub sample_rate: u64,
}
