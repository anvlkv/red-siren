use common::playback_quality::PlaybackQuality;

#[derive(Debug, Default, Clone, Copy, PartialEq, PartialOrd)]
pub enum PlaybackQualityGate {
    Ultra = 2,
    HiFi = 1,
    #[default]
    Medium = 0,
    LoFi = -1,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, PartialOrd)]
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
    pub fn lower(&self) -> Self {
        match self {
            Self::Ultra => Self::HiFi,
            Self::HiFi => Self::Medium,
            Self::Medium => Self::LoFi,
            Self::LoFi => Self::LoFi,
        }
    }

    pub fn higher(&self) -> Self {
        match self {
            Self::Ultra => Self::Ultra,
            Self::HiFi => Self::Ultra,
            Self::Medium => Self::HiFi,
            Self::LoFi => Self::Medium,
        }
    }

    pub fn sample_type(&self) -> SampleType {
        if matches!(self, Self::HiFi | Self::Ultra) {
            SampleType::F64
        } else {
            SampleType::F32
        }
    }
}

impl From<PlaybackQualityGate> for PlaybackQuality {
    fn from(val: PlaybackQualityGate) -> Self {
        match val {
            PlaybackQualityGate::Ultra => PlaybackQuality::Ultra,
            PlaybackQualityGate::HiFi => PlaybackQuality::HiFi,
            PlaybackQualityGate::Medium => PlaybackQuality::Medium,
            PlaybackQualityGate::LoFi => PlaybackQuality::LoFi,
        }
    }
}

impl From<PlaybackQuality> for PlaybackQualityGate {
    fn from(value: PlaybackQuality) -> Self {
        match value {
            PlaybackQuality::Auto(mode) => Self::from(mode),
            PlaybackQuality::LoFi => Self::LoFi,
            PlaybackQuality::Medium => Self::Medium,
            PlaybackQuality::HiFi => Self::HiFi,
            PlaybackQuality::Ultra => Self::Ultra,
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

impl From<cpal::SupportedStreamConfig> for PlaybackQualityGate {
    fn from(value: cpal::SupportedStreamConfig) -> PlaybackQualityGate {
        match value.sample_rate() {
            ..MID_SAMPLE_RATE => PlaybackQualityGate::LoFi,
            MID_SAMPLE_RATE..HI_SAMPLE_RATE => PlaybackQualityGate::Medium,
            HI_SAMPLE_RATE..ULTRA_SAMPLE_RATE => PlaybackQualityGate::HiFi,
            ULTRA_SAMPLE_RATE.. => PlaybackQualityGate::Ultra,
        }
    }
}

impl PlaybackQualityGate {
    pub fn buffer_size(&self, cfg: Option<&cpal::SupportedStreamConfigRange>) -> u32 {
        let sr_ms = self.sample_rate(cfg) as f64 / 1000.0;
        let ultra_buffer = (ULTRA_BUFFER_MS as f64 * sr_ms).round() as u32;
        let hi_buffer = (HI_BUFFER_MS as f64 * sr_ms).round() as u32;
        let mid_buffer = (MID_BUFFER_MS as f64 * sr_ms).round() as u32;
        let lo_buffer = (LO_BUFFER_MS as f64 * sr_ms).round() as u32;

        let mapper =
            |c: &cpal::SupportedStreamConfigRange, default_buffer: u32| match c.buffer_size() {
                cpal::SupportedBufferSize::Range { min, max } => ultra_buffer.clamp(*min, *max),
                cpal::SupportedBufferSize::Unknown => default_buffer,
            };

        return match self {
            PlaybackQualityGate::Ultra => cfg.map_or(ultra_buffer, |c| mapper(c, ultra_buffer)),
            PlaybackQualityGate::HiFi => cfg.map_or(hi_buffer, |c| mapper(c, hi_buffer)),
            PlaybackQualityGate::Medium => cfg.map_or(mid_buffer, |c| mapper(c, mid_buffer)),
            PlaybackQualityGate::LoFi => cfg.map_or(lo_buffer, |c| mapper(c, lo_buffer)),
        };
    }

    pub fn sample_rate(&self, cfg: Option<&cpal::SupportedStreamConfigRange>) -> u32 {
        return match self {
            PlaybackQualityGate::Ultra => cfg.map_or(ULTRA_SAMPLE_RATE, |c| {
                ULTRA_SAMPLE_RATE.clamp(c.min_sample_rate(), c.max_sample_rate())
            }),
            PlaybackQualityGate::HiFi => cfg.map_or(HI_SAMPLE_RATE, |c| {
                HI_SAMPLE_RATE.clamp(c.min_sample_rate(), c.max_sample_rate())
            }),
            PlaybackQualityGate::Medium => cfg.map_or(MID_SAMPLE_RATE, |c| {
                MID_SAMPLE_RATE.clamp(c.min_sample_rate(), c.max_sample_rate())
            }),
            PlaybackQualityGate::LoFi => cfg.map_or(LO_SAMPLE_RATE, |c| {
                LO_SAMPLE_RATE.clamp(c.min_sample_rate(), c.max_sample_rate())
            }),
        };
    }
}

impl PlaybackQualityGate {
    pub fn select_output_config(
        &self,
        device: &cpal::Device,
    ) -> Option<cpal::SupportedStreamConfig> {
        use cpal::traits::DeviceTrait;

        device
            .supported_output_configs()
            .ok()
            .and_then(|mut cfgs| {
                let mut pick = cfgs.next();
                for other in cfgs {
                    let a = &other;
                    let b = pick.as_ref().unwrap();
                    if self.is_a_better_than_b(a, b) {
                        pick = Some(other);
                    }
                }
                pick.map(|p| p.with_sample_rate(self.sample_rate(Some(&p))))
            })
            .or_else(|| device.default_output_config().ok())
    }

    pub fn select_input_config(
        &self,
        device: &cpal::Device,
    ) -> Option<cpal::SupportedStreamConfig> {
        use cpal::traits::DeviceTrait;

        device
            .supported_input_configs()
            .ok()
            .and_then(|mut cfgs| {
                let mut pick = cfgs.next();
                for other in cfgs {
                    let a = &other;
                    let b = pick.as_ref().unwrap();
                    if self.is_a_better_than_b(a, b) {
                        pick = Some(other);
                    }
                }
                pick.map(|p| p.with_sample_rate(self.sample_rate(Some(&p))))
            })
            .or_else(|| device.default_input_config().ok())
    }

    fn is_a_better_than_b(
        &self,
        a: &cpal::SupportedStreamConfigRange,
        b: &cpal::SupportedStreamConfigRange,
    ) -> bool {
        let target_sr = self.sample_rate(None);
        let fallback_sr = self
            .sample_rate(Some(a))
            .max(self.sample_rate(Some(b)))
            .min(target_sr);

        let target_buffer = self.buffer_size(None);
        let fallback_buffer = self
            .buffer_size(Some(a))
            .min(self.buffer_size(Some(b)))
            .max(target_buffer);

        let a_sr = a.min_sample_rate()..=a.max_sample_rate();
        let b_sr = b.min_sample_rate()..=b.max_sample_rate();
        let a_contains = a_sr.contains(&target_sr);
        let b_contains = b_sr.contains(&target_sr);

        let score = |bs: &cpal::SupportedBufferSize| -> i64 {
            match bs {
                cpal::SupportedBufferSize::Range { min, max } => {
                    let rng = *min..=*max;
                    if rng.contains(&target_buffer) {
                        0
                    } else if rng.contains(&fallback_buffer) {
                        1
                    } else {
                        2
                    }
                }
                cpal::SupportedBufferSize::Unknown => 3,
            }
        };

        if b_contains && !a_contains {
            true
        } else if a_contains && !b_contains {
            false
        } else if a_contains && b_contains {
            score(b.buffer_size()) > score(a.buffer_size())
        } else {
            let dist_b = {
                let min = b.min_sample_rate();
                let max = b.max_sample_rate();
                (fallback_sr.clamp(min, max) as i64 - target_sr as i64).abs()
            };
            let dist_a = {
                let min = a.min_sample_rate();
                let max = a.max_sample_rate();
                (fallback_sr.clamp(min, max) as i64 - target_sr as i64).abs()
            };

            if dist_a == dist_b {
                score(b.buffer_size()) > score(a.buffer_size())
            } else {
                dist_a < dist_b
            }
        }
    }
}
