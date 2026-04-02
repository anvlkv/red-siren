use std::{collections::VecDeque, sync::Arc, time::Duration};

use fundsp::thingbuf::mpsc::{Receiver, Sender, channel};

use crate::quality::{PlaybackQualityGate, SampleType};

use super::ProcessingMode;

pub type TelemetrySender = Arc<Sender<Message>>;
pub type TelemetryReceiver = Arc<Receiver<Message>>;

pub fn create_telemetry_channel() -> (TelemetrySender, TelemetryReceiver) {
    let (sx, rx) = channel(128);
    (Arc::new(sx), Arc::new(rx))
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Message {
    pub mode: ProcessingMode,
    pub filled_size: usize,
    pub buffer_size: usize,
    pub processing_time: Duration,
    pub estimated_latency: Option<Duration>,
    pub quality: PlaybackQualityGate,
    pub sample_type: SampleType,
}

impl Message {
    fn buffer_duration(&self, sample_rate: u32) -> Duration {
        Duration::from_secs_f64(self.buffer_size as f64 / sample_rate as f64)
    }

    fn filled_duration(&self, sample_rate: u32) -> Duration {
        Duration::from_secs_f64(self.filled_size as f64 / sample_rate as f64)
    }
}

enum BufferSize {
    Fixed(usize),
    Dynamic { size: usize, recompute_in: usize },
}

pub struct PlaybackTelemetry {
    history: VecDeque<Message>,
    sample_rate: u32,
    sample_type: SampleType,
    quality: PlaybackQualityGate,
    buffer_size: BufferSize,
    history_len: usize,
}

impl PlaybackTelemetry {
    // Durations in seconds
    const HISTORY_DURATION_S: f64 = 3.0;
    const SUSTAIN_S: f64 = Self::HISTORY_DURATION_S * 0.25;
    const DEGRADE_S: f64 = Self::HISTORY_DURATION_S * 0.1;
    const UPGRADE_S: f64 = Self::HISTORY_DURATION_S * 0.75;

    // Thresholds in %
    const DEGRADE_THR: f64 = 0.85;
    const UPGRADE_THR: f64 = 0.6;

    pub fn new(
        sample_rate: u32,
        sample_type: SampleType,
        quality: PlaybackQualityGate,
        buffer_size: Option<usize>,
    ) -> Self {
        let initial_buffer_size = buffer_size.unwrap_or_else(|| {
            quality.buffer_size(
                #[cfg(feature = "rt_cpal")]
                None,
            ) as usize
        });
        let history_len =
            Self::history_size(sample_rate, initial_buffer_size, Self::HISTORY_DURATION_S);

        let buffer_size = buffer_size.map_or(
            BufferSize::Dynamic {
                size: initial_buffer_size,
                recompute_in: history_len,
            },
            BufferSize::Fixed,
        );

        Self {
            history: VecDeque::with_capacity(history_len),
            sample_rate,
            sample_type,
            quality,
            buffer_size,
            history_len,
        }
    }

    pub fn accept_message(&mut self, msg: Message) -> Option<PlaybackQualityGate> {
        let buffer_size = match &mut self.buffer_size {
            BufferSize::Fixed(s) => *s,
            BufferSize::Dynamic { size, recompute_in } => {
                if let Some(r) = recompute_in.checked_sub(1) {
                    *recompute_in = r;
                    *size
                } else {
                    let average_size = (self.history.iter().map(|m| m.buffer_size).sum::<usize>()
                        as f64)
                        / self.history.len() as f64;
                    *recompute_in = self.history_len;
                    *size = average_size.round() as usize;
                    *size
                }
            }
        };

        let history_len =
            Self::history_size(self.sample_rate, buffer_size, Self::HISTORY_DURATION_S);

        if self.history_len != history_len {
            self.history_len = history_len;
            self.history.reserve(history_len);
        }

        self.history.push_back(msg);

        if self.history.len() < self.history_len {
            return None;
        }

        while self.history_len < self.history.len() {
            _ = self.history.pop_front();
        }

        let sustain_slice_size = Self::history_size(self.sample_rate, buffer_size, Self::SUSTAIN_S);

        if Self::should_sustain(
            self.history.iter().take(sustain_slice_size).rev(),
            self.quality,
        ) {
            return None;
        }

        let degrade_slice_size = Self::history_size(self.sample_rate, buffer_size, Self::DEGRADE_S);

        if Self::should_degrade(
            self.history
                .iter()
                .filter(|msg| msg.sample_type >= self.sample_type)
                .take(degrade_slice_size),
            self.quality,
            self.sample_rate,
        ) {
            return Some(self.quality.lower());
        }

        let upgrade_slice_size = Self::history_size(self.sample_rate, buffer_size, Self::UPGRADE_S);

        if Self::should_upgrade(
            self.history
                .iter()
                .filter(|msg| msg.sample_type <= self.sample_type)
                .take(upgrade_slice_size),
            self.quality,
            self.sample_rate,
        ) {
            return Some(self.quality.higher());
        }

        None
    }

    pub fn update(
        &mut self,
        sample_rate: u32,
        sample_type: SampleType,
        quality: PlaybackQualityGate,
        buffer_size: Option<usize>,
    ) {
        let initial_buffer_size = buffer_size.unwrap_or_else(|| {
            quality.buffer_size(
                #[cfg(feature = "rt_cpal")]
                None,
            ) as usize
        });
        let history_len =
            Self::history_size(sample_rate, initial_buffer_size, Self::HISTORY_DURATION_S);

        let buffer_size = buffer_size.map_or(
            BufferSize::Dynamic {
                size: initial_buffer_size,
                recompute_in: history_len,
            },
            BufferSize::Fixed,
        );

        self.sample_rate = sample_rate;
        self.sample_type = sample_type;
        self.quality = quality;
        self.buffer_size = buffer_size;
        self.history_len = history_len;
    }

    fn should_sustain<'m, I>(mut history: I, current_quality: PlaybackQualityGate) -> bool
    where
        I: Iterator<Item = &'m Message>,
    {
        history.any(|msg| msg.quality != current_quality)
    }

    fn should_degrade<'m, I>(
        history: I,
        current_quality: PlaybackQualityGate,
        sample_rate: u32,
    ) -> bool
    where
        I: Iterator<Item = &'m Message>,
    {
        let (
            processing_time,
            buffers_duration,
            filled_duration,
            estimated_latency,
            catch_up_count,
            len,
        ) = history.filter(|m| m.quality >= current_quality).fold(
            (
                Duration::ZERO,
                Duration::ZERO,
                Duration::ZERO,
                Duration::ZERO,
                0_usize,
                0_usize,
            ),
            |(
                processing_time,
                buffers_duration,
                filled_duration,
                estimated_latency,
                catch_up_count,
                len,
            ),
             msg| {
                (
                    processing_time + msg.processing_time,
                    buffers_duration + msg.buffer_duration(sample_rate),
                    filled_duration + msg.filled_duration(sample_rate),
                    estimated_latency + msg.estimated_latency.unwrap_or(Duration::ZERO),
                    catch_up_count
                        + if matches!(msg.mode, ProcessingMode::Tick) {
                            0
                        } else {
                            1
                        },
                    len + 1,
                )
            },
        );

        let processing_to_filled =
            safe_ratio(processing_time.as_secs_f64(), filled_duration.as_secs_f64());
        let latency_to_buffers = safe_ratio(
            estimated_latency.as_secs_f64(),
            buffers_duration.as_secs_f64(),
        );
        let processing_to_buffer = safe_ratio(
            processing_time.as_secs_f64(),
            buffers_duration.as_secs_f64(),
        );
        let catch_up_ratio = safe_ratio(catch_up_count as f64, len as f64);

        processing_to_buffer.is_some_and(|s| s > Self::DEGRADE_THR)
            || latency_to_buffers
                .zip(processing_to_filled)
                .is_some_and(|(lb, pf)| lb > (1.0 - Self::DEGRADE_THR) && pf > Self::DEGRADE_THR)
            || catch_up_ratio.is_some_and(|r| r > Self::DEGRADE_THR)
    }

    fn should_upgrade<'m, I>(
        history: I,
        current_quality: PlaybackQualityGate,
        sample_rate: u32,
    ) -> bool
    where
        I: Iterator<Item = &'m Message>,
    {
        let (
            processing_time,
            buffers_duration,
            filled_duration,
            estimated_latency,
            catch_up_count,
            len,
        ) = history.filter(|m| m.quality <= current_quality).fold(
            (
                Duration::ZERO,
                Duration::ZERO,
                Duration::ZERO,
                Duration::ZERO,
                0_usize,
                0_usize,
            ),
            |(
                processing_time,
                buffers_duration,
                filled_duration,
                estimated_latency,
                catch_up_count,
                len,
            ),
             msg| {
                (
                    processing_time + msg.processing_time,
                    buffers_duration + msg.buffer_duration(sample_rate),
                    filled_duration + msg.filled_duration(sample_rate),
                    estimated_latency + msg.estimated_latency.unwrap_or(Duration::ZERO),
                    catch_up_count
                        + if matches!(msg.mode, ProcessingMode::Tick) {
                            0
                        } else {
                            1
                        },
                    len + 1,
                )
            },
        );

        let processing_to_filled =
            safe_ratio(processing_time.as_secs_f64(), filled_duration.as_secs_f64());
        let latency_to_buffers = safe_ratio(
            estimated_latency.as_secs_f64(),
            buffers_duration.as_secs_f64(),
        );
        let processing_to_buffer = safe_ratio(
            processing_time.as_secs_f64(),
            buffers_duration.as_secs_f64(),
        );
        let catch_up_ratio = safe_ratio(catch_up_count as f64, len as f64);

        (processing_to_buffer.is_some_and(|s| s < Self::UPGRADE_THR)
            || latency_to_buffers
                .zip(processing_to_filled)
                .is_some_and(|(lb, pf)| lb < (1.0 - Self::UPGRADE_THR) && pf < Self::UPGRADE_THR))
            && catch_up_ratio.is_none_or(|r| r < (1.0 - Self::UPGRADE_THR))
    }

    fn history_size(sample_rate: u32, buffer_size: usize, target_duration: f64) -> usize {
        ((target_duration * sample_rate as f64) / buffer_size as f64).ceil() as usize
    }
}

fn safe_ratio(num: f64, denom: f64) -> Option<f64> {
    if denom <= 0.0 {
        None
    } else {
        Some(num / denom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(
        quality: PlaybackQualityGate,
        buffer_frames: usize,
        filled_frames: usize,
        processing_ms: u64,
        estimated_latency_ms: Option<u64>,
    ) -> Message {
        Message {
            mode: ProcessingMode::Tick,
            buffer_size: buffer_frames,
            filled_size: filled_frames,
            processing_time: Duration::from_millis(processing_ms),
            estimated_latency: estimated_latency_ms.map(Duration::from_millis),
            quality,
            sample_type: SampleType::F32,
        }
    }

    fn telemetry(
        sr: u32,
        quality: PlaybackQualityGate,
        fixed_buffer_frames: Option<usize>,
    ) -> PlaybackTelemetry {
        PlaybackTelemetry::new(sr, SampleType::F32, quality, fixed_buffer_frames)
    }

    // Fast tests with tiny sample_rate and buffer
    #[test]
    fn fast_no_decision_until_history_filled() {
        let sr = 100; // very small sample rate
        let buffer_frames = 10; // 0.1s buffer at 100 Hz
        let mut t = telemetry(sr, PlaybackQualityGate::Medium, Some(buffer_frames));
        // HISTORY_DURATION_S = 3s -> history_len = ceil(3.0 / 0.1) = 30
        for i in 0..29 {
            let m = msg(
                PlaybackQualityGate::Medium,
                buffer_frames,
                buffer_frames,
                1,
                Some(1),
            );
            assert_eq!(t.accept_message(m), None, "unexpected decision at i={}", i);
        }
        // Fill the window
        let m = msg(
            PlaybackQualityGate::Medium,
            buffer_frames,
            buffer_frames,
            1,
            Some(1),
        );
        assert_eq!(t.accept_message(m), None);
    }

    #[test]
    fn fast_sustain_prevents_change_with_mixed_recent_qualities() {
        let sr = 100;
        let buffer_frames = 10;
        let mut t = telemetry(sr, PlaybackQualityGate::Medium, Some(buffer_frames));
        let history_len = 30;
        for _ in 0..history_len {
            let m = msg(
                PlaybackQualityGate::Medium,
                buffer_frames,
                buffer_frames,
                1,
                Some(1),
            );
            t.accept_message(m);
        }
        // SUSTAIN_S = 0.75s -> slice size ~ ceil(0.75 / 0.1) = 8
        for _ in 0..8 {
            let m = msg(
                PlaybackQualityGate::HiFi,
                buffer_frames,
                buffer_frames,
                1,
                Some(1),
            );
            let decision = t.accept_message(m);
            assert_eq!(decision, None);
        }
    }

    #[test]
    fn fast_degrade_on_high_processing_to_buffer() {
        let sr = 100;
        let buffer_frames = 10; // 0.1s per buffer
        let mut t = telemetry(sr, PlaybackQualityGate::HiFi, Some(buffer_frames));
        // Make processing_time ~0.09s per msg => ratio ~0.9 > DEGRADE_THR 0.85
        let mut got = false;
        for _ in 0..30 {
            let m = msg(
                PlaybackQualityGate::HiFi,
                buffer_frames,
                buffer_frames,
                90,
                Some(0),
            );
            if let Some(decision) = t.accept_message(m) {
                assert_eq!(decision, PlaybackQualityGate::Medium);
                got = true;
                break;
            }
        }
        assert!(got, "Expected degrade recommendation");
    }

    #[test]
    fn fast_degrade_by_combined_high_latency_and_processing_to_filled() {
        let sr = 100;
        let buffer_frames = 10; // 0.1s
        let mut t = telemetry(sr, PlaybackQualityGate::HiFi, Some(buffer_frames));
        // processing_to_filled ~0.9 and latency_to_buffers ~0.2 > 0.15
        let mut got = false;
        for _ in 0..30 {
            let m = msg(
                PlaybackQualityGate::HiFi,
                buffer_frames,
                buffer_frames,
                90,
                Some(20),
            );
            if let Some(decision) = t.accept_message(m) {
                assert_eq!(decision, PlaybackQualityGate::Medium);
                got = true;
                break;
            }
        }
        assert!(got, "Expected degrade via combined condition");
    }

    #[test]
    fn fast_upgrade_on_low_processing_to_buffer() {
        let sr = 100;
        let buffer_frames = 10; // 0.1s
        let mut t = telemetry(sr, PlaybackQualityGate::Medium, Some(buffer_frames));
        // processing_to_buffer 0.03 / 0.1 = 0.3 < 0.6
        let mut got = false;
        for _ in 0..30 {
            let m = msg(
                PlaybackQualityGate::Medium,
                buffer_frames,
                buffer_frames,
                30,
                Some(0),
            );
            if let Some(decision) = t.accept_message(m) {
                assert_eq!(decision, PlaybackQualityGate::HiFi);
                got = true;
                break;
            }
        }
        assert!(got, "Expected upgrade recommendation");
    }

    #[test]
    fn fast_upgrade_by_combined_low_latency_and_processing_to_filled() {
        let sr = 100;
        let buffer_frames = 10; // 0.1s
        let mut t = telemetry(sr, PlaybackQualityGate::Medium, Some(buffer_frames));
        // processing_to_filled 0.05 / 0.1 = 0.5 < 0.6; latency_to_buffers 0.03 / 0.1 = 0.3 < 0.4
        let mut got = false;
        for _ in 0..30 {
            let m = msg(
                PlaybackQualityGate::Medium,
                buffer_frames,
                buffer_frames,
                50,
                Some(30),
            );
            if let Some(decision) = t.accept_message(m) {
                assert_eq!(decision, PlaybackQualityGate::HiFi);
                got = true;
                break;
            }
        }
        assert!(got, "Expected upgrade via combined condition");
    }

    #[test]
    fn fast_zero_denominators_do_not_trigger_changes() {
        let sr = 100;
        let buffer_frames = 10;
        let mut t = telemetry(sr, PlaybackQualityGate::Medium, Some(buffer_frames));
        // filled_frames = 0 => filled_duration = 0 => processing_to_filled = None
        // Ensure no false decisions
        for _ in 0..30 {
            let m = msg(PlaybackQualityGate::Medium, buffer_frames, 0, 1, Some(0));
            assert_eq!(t.accept_message(m), None);
        }
    }

    #[test]
    fn fast_dynamic_buffer_recompute_smoke() {
        let sr = 100;
        // Dynamic buffer_size path: None => derive from quality
        let mut t = telemetry(sr, PlaybackQualityGate::Medium, None);
        // Alternate buffer sizes to force average update
        for i in 0..60 {
            let variable_buffer = if i % 2 == 0 { 10 } else { 20 }; // 0.1s and 0.2s
            let m = msg(
                PlaybackQualityGate::Medium,
                variable_buffer,
                variable_buffer,
                10,
                Some(0),
            );
            let _ = t.accept_message(m);
        }
        // No panic means recompute and history_len adjustment worked
    }

    #[test]
    fn fast_gate_bounds_clamp() {
        let sr = 100;
        let buffer_frames = 10;
        let mut t_lo = telemetry(sr, PlaybackQualityGate::LoFi, Some(buffer_frames));
        let mut saw_lo = false;
        for _ in 0..30 {
            let m = msg(
                PlaybackQualityGate::LoFi,
                buffer_frames,
                buffer_frames,
                90,
                Some(20),
            );
            if let Some(decision) = t_lo.accept_message(m) {
                assert_eq!(decision, PlaybackQualityGate::LoFi);
                saw_lo = true;
                break;
            }
        }
        assert!(saw_lo, "LoFi should clamp on lower()");
        let mut t_hi = telemetry(sr, PlaybackQualityGate::Ultra, Some(buffer_frames));
        let mut saw_hi = false;
        for _ in 0..30 {
            let m = msg(
                PlaybackQualityGate::Ultra,
                buffer_frames,
                buffer_frames,
                10,
                Some(0),
            );
            if let Some(decision) = t_hi.accept_message(m) {
                assert_eq!(decision, PlaybackQualityGate::Ultra);
                saw_hi = true;
                break;
            }
        }
        assert!(saw_hi, "Ultra should clamp on higher()");
    }

    // One realistic test with 48kHz
    #[test]
    fn realistic_upgrade_under_low_load() {
        let sr = 48_000;
        let buffer_frames = 480; // 10ms buffer
        let mut t = telemetry(sr, PlaybackQualityGate::Medium, Some(buffer_frames));
        // processing_to_buffer < 0.6 -> 3ms processing
        let mut got = false;
        // HISTORY_DURATION_S = 3s -> history_len = 300 messages
        for _ in 0..300 {
            let m = msg(
                PlaybackQualityGate::Medium,
                buffer_frames,
                buffer_frames,
                3,
                Some(0),
            );
            if let Some(decision) = t.accept_message(m) {
                assert_eq!(decision, PlaybackQualityGate::HiFi);
                got = true;
                break;
            }
        }
        assert!(got, "Expected upgrade with realistic parameters");
    }
}
