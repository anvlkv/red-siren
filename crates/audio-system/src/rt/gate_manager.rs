use std::time::{Duration, Instant};

use crate::quality::PlaybackQualityGate;
use crate::rt::cpal::stream::telemetry::{
    CHANGE_COOLDOWN_MS, DEGRADE_SUSTAIN_MS, DEGRADE_THRESHOLD_MS, UPGRADE_STABLE_MS,
    UPGRADE_THRESHOLD_MS,
};

/// GateManager evaluates telemetry-derived signals to recommend raising or lowering
/// the `PlaybackQualityGate`, with cooldowns to prevent flapping.
///
/// Why:
/// - Centralize upgrade/downgrade policy off the audio thread.
/// - Degrade quickly on sustained trouble; upgrade slowly on proven stability.
/// - Keep overhead minimal (simple time checks and scalar comparisons).
pub struct GateManager {
    /// Last time a gate change was applied (used for cooldown).
    last_change: Instant,
    /// Minimum duration to wait between changes.
    cooldown: Duration,
    /// Start time of the last stability period (for upgrade timing).
    stable_since: Option<Instant>,
    /// Start time when sustained trouble began (for degrade timing).
    trouble_since: Option<Instant>,
    /// Latest recommended gate, updated when evaluation suggests a change.
    pub recommended_gate: PlaybackQualityGate,
}

impl GateManager {
    /// Create a new manager with initial gate and default cooldown.
    pub fn new(initial_gate: PlaybackQualityGate) -> Self {
        Self {
            last_change: Instant::now(),
            cooldown: Duration::from_millis(CHANGE_COOLDOWN_MS),
            stable_since: None,
            trouble_since: None,
            recommended_gate: initial_gate,
        }
    }

    /// Evaluate whether to recommend a gate change based on slack and underruns.
    ///
    /// Inputs:
    /// - `current_gate`: the active gate in the engine.
    /// - `ema_compute_slack_ns`: smoothed slack (expected period - render time), in nanoseconds.
    /// - `recent_underruns`: whether we observed underruns in recent callbacks.
    /// - `now`: current time for cooldown/stability accounting.
    ///
    /// Returns:
    /// - `Some(new_gate)` when a change is recommended.
    /// - `None` when staying with `current_gate` is advised.
    ///
    /// Policy:
    /// - Degrade fast if sustained trouble:
    ///   - Slack < -3 ms for > 1 s, or repeated underruns.
    /// - Upgrade slow on stability:
    ///   - Slack ≥ +2 ms and zero underruns sustained for ≥ 20 s.
    /// - Apply a 5 s cooldown between changes to avoid flapping.
    pub fn evaluate_recommendation(
        &mut self,
        current_gate: PlaybackQualityGate,
        ema_compute_slack_ns: f64,
        recent_underruns: bool,
        now: Instant,
    ) -> Option<PlaybackQualityGate> {
        let slack_ms = ema_compute_slack_ns / 1_000_000.0;
        let can_change = now.duration_since(self.last_change) >= self.cooldown;

        // Track trouble sustainment window
        let trouble = slack_ms < DEGRADE_THRESHOLD_MS || recent_underruns;
        if trouble && self.trouble_since.is_none() {
            // Start trouble clock
            self.trouble_since = Some(now);
        } else if !trouble {
            // Clear trouble tracking when conditions improve
            self.trouble_since = None;
        }

        // Track stability window for upgrades
        let stable = slack_ms >= UPGRADE_THRESHOLD_MS && !recent_underruns;
        if stable && self.stable_since.is_none() {
            self.stable_since = Some(now);
        } else if !stable {
            self.stable_since = None;
        }

        // Degrade quickly on sustained trouble (or immediately on persistent underruns) if cooldown allows
        if can_change
            && matches!(
                self.trouble_since,
                Some(since) if now.duration_since(since) >= Duration::from_millis(DEGRADE_SUSTAIN_MS)
            )
        {
            let new_gate = current_gate.lower();
            if new_gate != current_gate {
                self.last_change = now;
                self.stable_since = None;
                self.trouble_since = None;
                self.recommended_gate = new_gate;
                return Some(new_gate);
            }
        }

        // Upgrade slowly on stable conditions if cooldown allows
        if let Some(new_gate) = self
            .stable_since
            .as_ref()
            .filter(|&since| {
                can_change && now.duration_since(*since) >= Duration::from_millis(UPGRADE_STABLE_MS)
            })
            .and_then(|_| {
                let new_gate = current_gate.higher();
                if new_gate != current_gate {
                    Some(new_gate)
                } else {
                    None
                }
            })
        {
            self.last_change = now;
            self.stable_since = None;
            self.trouble_since = None;
            self.recommended_gate = new_gate;
            Some(new_gate)
        } else {
            None
        }
    }

    /// Manually set a new cooldown duration in milliseconds.
    ///
    /// Why: Allows runtime tuning without recompiling, if needed later.
    pub fn set_cooldown_ms(&mut self, ms: u64) {
        self.cooldown = Duration::from_millis(ms);
    }

    /// Reset internal timing trackers (useful after external forced reconfigurations).
    pub fn reset_timers(&mut self, now: Instant) {
        self.last_change = now;
        self.stable_since = None;
        self.trouble_since = None;
    }
}
