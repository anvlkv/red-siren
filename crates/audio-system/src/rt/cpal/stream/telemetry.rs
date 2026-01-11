use std::time::Duration;

/// Telemetry smoothing and policy tuning constants.
/// Kept here per project guidance to centralize adjustments.
///
/// Why: These values balance responsiveness and stability. We keep them
/// conservative to avoid flapping and to prefer quality in steady state.
pub const STEADY_OCCUPANCY_RATIO: f64 = 0.80; // Target 80% of buffer capacity
pub const CATCH_UP_LATENCY_RATIO: f64 = 0.90; // Enter catch-up when estimated latency >= 90% of buffer duration

pub const EMA_RENDER_ALPHA: f64 = 0.20; // Render time EMA smoothing
pub const EMA_SLACK_ALPHA: f64 = 0.30;  // Slack EMA smoothing

// Gate management policy thresholds (engine-side consumption)
pub const DEGRADE_THRESHOLD_MS: f64 = -3.0; // Slack less than -3 ms triggers degrade
pub const DEGRADE_SUSTAIN_MS: u64 = 1000;   // Trouble must persist for at least 1 second
pub const UPGRADE_THRESHOLD_MS: f64 = 2.0;  // Slack greater than +2 ms enables upgrade
pub const UPGRADE_STABLE_MS: u64 = 20_000;  // Stability must persist for at least 20 seconds
pub const CHANGE_COOLDOWN_MS: u64 = 5_000;  // Minimum cooldown between changes

/// Playback telemetry shared between the audio callback and engine.
///
/// Why: The audio thread writes compact measurements; the engine reads
/// periodically to decide quality gate adjustments and to guide production mode.
///
/// Notes:
/// - Latency is estimated from queue depth and net latency (in frames), converted to ms.
/// - Slack compares expected callback period against measured render time; negative indicates falling behind.
/// - EMAs smooth jitter for more stable decisions; raw metrics are too noisy for direct gating.
#[derive(Debug, Default, Clone)]
pub struct PlaybackTelemetry {
    // Device/sample context
    pub sample_rate: u32,

    // Per-callback measurements
    pub last_callback_frames: usize,
    pub queue_depth_frames: usize,

    // Smoothed performance
    pub ema_render_ns_per_frame: f64,
    pub ema_compute_slack_ns: f64,

    // Latency model and estimate
    pub net_latency_frames: usize,
    pub estimated_latency_ms: f64,

    // Health counters
    pub local_underruns: u32,
    pub total_underruns: u64,
}

impl PlaybackTelemetry {
    /// Exponential moving average helper.
    #[inline]
    pub fn ema(cur: f64, sample: f64, alpha: f64) -> f64 {
        if cur == 0.0 {
            sample
        } else {
            alpha * sample + (1.0 - alpha) * cur
        }
    }

    /// Note render elapsed for a given number of frames; updates EMA for ns/frame and stores last callback size.
    ///
    /// Why: Render time per frame is the base signal used to compute slack against expected device pacing.
    #[inline]
    pub fn note_render(&mut self, frames: usize, elapsed: Duration) {
        if frames == 0 {
            return;
        }
        let ns_per_frame = (elapsed.as_nanos() as f64) / frames as f64;
        self.ema_render_ns_per_frame =
            Self::ema(self.ema_render_ns_per_frame, ns_per_frame, EMA_RENDER_ALPHA);
        self.last_callback_frames = frames;
    }

    /// Recompute slack against expected callback period.
    ///
    /// expected_period: the device-driven period for this callback (duration for `last_callback_frames` frames).
    ///
    /// Why: Slack indicates whether production is keeping up. Negative slack means we are falling behind.
    #[inline]
    pub fn recompute_slack(&mut self, expected_period: Duration) {
        let expected_ns = expected_period.as_nanos() as f64;
        let render_ns = self.ema_render_ns_per_frame * self.last_callback_frames as f64;
        let slack = expected_ns - render_ns;
        self.ema_compute_slack_ns =
            Self::ema(self.ema_compute_slack_ns, slack, EMA_SLACK_ALPHA);
    }

    /// Recompute latency estimate from queue depth and net latency (both in frames).
    ///
    /// Why: Estimated latency guides catch-up decisions and informs gate management in the engine.
    #[inline]
    pub fn recompute_latency(&mut self) {
        if self.sample_rate == 0 {
            self.estimated_latency_ms = 0.0;
            return;
        }
        let q_ms = (self.queue_depth_frames as f64 / self.sample_rate as f64) * 1000.0;
        let net_ms = (self.net_latency_frames as f64 / self.sample_rate as f64) * 1000.0;
        self.estimated_latency_ms = q_ms + net_ms;
    }

    /// Convert nanoseconds to frame count, rounded, at current sample rate.
    ///
    /// Why: Translates slack (time) into a production frame budget for catch-up decisions.
    #[inline]
    pub fn frames_from_ns(&self, ns: f64) -> usize {
        if self.sample_rate == 0 {
            return 0;
        }
        ((ns / 1_000_000_000.0) * self.sample_rate as f64).round() as usize
    }

    /// Record underruns for this callback.
    ///
    /// Why: Repeated underruns are a strong signal for immediate catch-up and possible gate downgrade.
    #[inline]
    pub fn note_underruns(&mut self, local: u32) {
        self.local_underruns = local;
        self.total_underruns = self
            .total_underruns
            .saturating_add(local as u64);
    }
}
