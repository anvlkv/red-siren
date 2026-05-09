use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::NodeKey;

#[derive(Debug, Clone, Default, Copy, Serialize, Deserialize, PartialEq)]
pub struct NodePreset {
    pub key: NodeKey,
    pub band_value: f32,
    pub key_value: bool,
}

impl NodePreset {
    pub fn key_value_as_f32(&self) -> f32 {
        if self.key_value { 1.0 } else { 0.0 }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Preset(pub HashMap<NodeKey, NodePreset>);

impl Preset {
    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn add(&mut self, key: NodeKey, band_value: f32, key_value: bool) {
        let preset = NodePreset {
            key,
            band_value,
            key_value,
        };
        self.0.insert(key, preset);
    }

    pub fn entry(&mut self, &key: &NodeKey) -> &mut NodePreset {
        self.0.entry(key).or_insert(NodePreset {
            key,
            ..Default::default()
        })
    }

    pub fn has(&self, key: &NodeKey) -> bool {
        self.0.contains_key(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &NodeKey> {
        self.0.keys()
    }

    pub fn get(&self, key: &NodeKey) -> Option<&NodePreset> {
        self.0.get(key)
    }

    pub fn get_key_value(&self, key: &NodeKey) -> Option<f32> {
        self.0.get(key).map(|preset| preset.key_value_as_f32())
    }

    pub fn get_band_value(&self, key: &NodeKey) -> Option<f32> {
        self.0.get(key).map(|preset| preset.band_value)
    }

    pub fn set_band_value(&mut self, key: &NodeKey, band_value: f32) -> bool {
        if let Some(preset) = self.0.get_mut(key) {
            preset.band_value = band_value;
            true
        } else {
            false
        }
    }

    pub fn set_key_value(&mut self, key: &NodeKey, key_value: f32) -> bool {
        if let Some(preset) = self.0.get_mut(key) {
            preset.key_value = key_value >= 0.5;
            true
        } else {
            false
        }
    }

    pub fn remove(&mut self, key: &NodeKey) -> Option<NodePreset> {
        self.0.remove(key)
    }
}

impl serde::Serialize for Preset {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialize as a simple list of NodePreset to avoid non-string map key issues
        let items: Vec<NodePreset> = self.0.values().copied().collect();
        items.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for Preset {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Deserialize from a list of NodePreset and rebuild the HashMap keyed by NodeKey
        let items: Vec<NodePreset> = Vec::deserialize(deserializer)?;
        let map: std::collections::HashMap<NodeKey, NodePreset> =
            items.into_iter().map(|np| (np.key, np)).collect();
        Ok(Preset(map))
    }
}
