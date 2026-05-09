use crate::NodeKey;
use serde::{Deserialize, Serialize};

/// Perform a manual "hit" on a test node — sets excite, frequency and key-press in one call.
pub const TEST_NODE_HIT: &str = "test_node_hit";

/// Release a previously hit test node.
pub const TEST_NODE_RELEASE: &str = "test_node_release";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestNodeHitPayload {
    pub node_key: NodeKey,
    pub frequency: f32,
    pub excite_real: f32,
    pub excite_imag: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestNodeReleasePayload {
    pub node_key: NodeKey,
}
