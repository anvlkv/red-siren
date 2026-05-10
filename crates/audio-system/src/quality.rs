#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PlaybackQualityGate {
    Ultra = 2,
    HiFi = 1,
    #[default]
    Medium = 0,
    LoFi = -1,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SampleType {
    #[default]
    F32,
    F64,
}

const ULTRA_SAMPLE_RATE: u32 = 96_000;
const HI_SAMPLE_RATE: u32 = 48_000;
const MID_SAMPLE_RATE: u32 = 44_100;
const LO_SAMPLE_RATE: u32 = 32_000;

const ULTRA_BUFFER_MS: u32 = 32;
const HI_BUFFER_MS: u32 = 64;
const MID_BUFFER_MS: u32 = 128;
const LO_BUFFER_MS: u32 = 256;

impl PlaybackQualityGate {
    pub fn lower(self) -> Self {
        match self {
            Self::Ultra => Self::HiFi,
            Self::HiFi => Self::Medium,
            Self::Medium => Self::LoFi,
            Self::LoFi => Self::LoFi,
        }
    }

    pub fn higher(self) -> Self {
        match self {
            Self::Ultra => Self::Ultra,
            Self::HiFi => Self::Ultra,
            Self::Medium => Self::HiFi,
            Self::LoFi => Self::Medium,
        }
    }

    pub fn sample_type(self) -> SampleType {
        if matches!(self, Self::HiFi | Self::Ultra) {
            SampleType::F64
        } else {
            SampleType::F32
        }
    }

    pub fn sample_rate(self) -> u32 {
        match self {
            Self::Ultra => ULTRA_SAMPLE_RATE,
            Self::HiFi => HI_SAMPLE_RATE,
            Self::Medium => MID_SAMPLE_RATE,
            Self::LoFi => LO_SAMPLE_RATE,
        }
    }

    pub fn buffer_size(self) -> u32 {
        let sr_ms = self.sample_rate() as f64 / 1000.0;
        let ultra_buffer = (ULTRA_BUFFER_MS as f64 * sr_ms).round() as u32;
        let hi_buffer = (HI_BUFFER_MS as f64 * sr_ms).round() as u32;
        let mid_buffer = (MID_BUFFER_MS as f64 * sr_ms).round() as u32;
        let lo_buffer = (LO_BUFFER_MS as f64 * sr_ms).round() as u32;

        match self {
            Self::Ultra => ultra_buffer,
            Self::HiFi => hi_buffer,
            Self::Medium => mid_buffer,
            Self::LoFi => lo_buffer,
        }
    }
}

impl From<i8> for PlaybackQualityGate {
    fn from(value: i8) -> Self {
        match value {
            ..=-1 => Self::LoFi,
            0 => Self::Medium,
            1 => Self::HiFi,
            2.. => Self::Ultra,
        }
    }
}
