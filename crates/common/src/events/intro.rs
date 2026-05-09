use serde::{Deserialize, Serialize};

/// One snoop’s current snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntroSnoopSample {
    /// 1-based snoop identifier (1..=11).
    pub snoop_id: u8,
    /// Most recent time-domain samples (normalized -1.0..1.0).
    pub samples: Vec<f32>,
}

/// Batched payload containing all snoops for a single engine tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntroSnoopBatchPayload {
    /// Unix timestamp (ms) when the batch was produced.
    pub t_unix_ms: u64,
    /// Collection of snoop snapshots. Typically length == 11.
    pub snoops: Vec<IntroSnoopSample>,
}
