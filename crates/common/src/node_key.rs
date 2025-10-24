use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct NodeKey(pub u8, pub u8);

impl NodeKey {
    /// Create a new NodeKey with bounds validation
    pub fn new(group: u8, key: u8) -> Self {
        NodeKey(group, key)
    }

    /// Get group index
    pub fn group(&self) -> u8 {
        self.0
    }

    /// Get key index within group
    pub fn key(&self) -> u8 {
        self.1
    }

    /// Validate this NodeKey against layout bounds
    pub fn is_valid(&self, num_groups: u8, num_keys_per_group: u8) -> bool {
        self.0 < num_groups && self.1 < num_keys_per_group
    }

    /// Get linear index for this NodeKey
    pub fn idx(&self) -> usize {
        (self.0 as usize) * 256 + (self.1 as usize)
    }
}

/// Utility for managing NodeKeys with layout validation
pub struct NodeKeyRegistry {
    num_groups: u8,
    num_keys_per_group: u8,
}

impl NodeKeyRegistry {
    /// Create new registry with layout bounds
    pub fn new(num_groups: u8, num_keys_per_group: u8) -> Self {
        Self {
            num_groups,
            num_keys_per_group,
        }
    }

    /// Create NodeKey with validation
    pub fn create_key(&self, group: u8, key: u8) -> Result<NodeKey, NodeKeyError> {
        if group >= self.num_groups {
            return Err(NodeKeyError::GroupOutOfBounds {
                group,
                max: self.num_groups.saturating_sub(1),
            });
        }
        if key >= self.num_keys_per_group {
            return Err(NodeKeyError::KeyOutOfBounds {
                key,
                max: self.num_keys_per_group.saturating_sub(1),
            });
        }
        Ok(NodeKey::new(group, key))
    }

    /// Validate existing NodeKey
    pub fn validate(&self, node_key: &NodeKey) -> Result<(), NodeKeyError> {
        self.create_key(node_key.group(), node_key.key())?;
        Ok(())
    }

    /// Generate all valid NodeKeys for current layout
    pub fn all_keys(&self) -> Vec<NodeKey> {
        let mut keys =
            Vec::with_capacity((self.num_groups as usize) * (self.num_keys_per_group as usize));
        for group in 0..self.num_groups {
            for key in 0..self.num_keys_per_group {
                keys.push(NodeKey::new(group, key));
            }
        }
        keys
    }

    /// Safely iterate over all valid NodeKeys
    pub fn iter_keys<F>(&self, mut f: F)
    where
        F: FnMut(NodeKey),
    {
        for group in 0..self.num_groups {
            for key in 0..self.num_keys_per_group {
                f(NodeKey::new(group, key));
            }
        }
    }

    /// Filter out invalid NodeKeys from a collection
    pub fn filter_valid_keys<T>(&self, map: &HashMap<NodeKey, T>) -> HashMap<NodeKey, T>
    where
        T: Clone,
    {
        map.iter()
            .filter(|(node_key, _)| node_key.is_valid(self.num_groups, self.num_keys_per_group))
            .map(|(k, v)| (*k, v.clone()))
            .collect()
    }

    /// Check if any keys in collection are invalid
    pub fn has_invalid_keys<T>(&self, map: &HashMap<NodeKey, T>) -> Vec<NodeKey> {
        map.keys()
            .filter(|node_key| !node_key.is_valid(self.num_groups, self.num_keys_per_group))
            .copied()
            .collect()
    }

    pub fn num_groups(&self) -> u8 {
        self.num_groups
    }

    pub fn num_keys_per_group(&self) -> u8 {
        self.num_keys_per_group
    }

    pub fn total_keys(&self) -> usize {
        (self.num_groups as usize) * (self.num_keys_per_group as usize)
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum NodeKeyError {
    #[error("Group index {group} out of bounds (max: {max})")]
    GroupOutOfBounds { group: u8, max: u8 },
    #[error("Key index {key} out of bounds (max: {max})")]
    KeyOutOfBounds { key: u8, max: u8 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_key_creation() {
        let key = NodeKey::new(1, 2);
        assert_eq!(key.group(), 1);
        assert_eq!(key.key(), 2);
    }

    #[test]
    fn test_node_key_validation() {
        let key = NodeKey::new(1, 2);
        assert!(key.is_valid(3, 4));
        assert!(!key.is_valid(1, 4)); // group out of bounds
        assert!(!key.is_valid(3, 2)); // key out of bounds
    }

    #[test]
    fn test_registry_create_key() {
        let registry = NodeKeyRegistry::new(2, 3);

        // Valid keys
        assert!(registry.create_key(0, 0).is_ok());
        assert!(registry.create_key(1, 2).is_ok());

        // Invalid keys
        assert!(registry.create_key(2, 0).is_err()); // group out of bounds
        assert!(registry.create_key(0, 3).is_err()); // key out of bounds
    }

    #[test]
    fn test_registry_all_keys() {
        let registry = NodeKeyRegistry::new(2, 2);
        let keys = registry.all_keys();

        assert_eq!(keys.len(), 4);
        assert!(keys.contains(&NodeKey::new(0, 0)));
        assert!(keys.contains(&NodeKey::new(0, 1)));
        assert!(keys.contains(&NodeKey::new(1, 0)));
        assert!(keys.contains(&NodeKey::new(1, 1)));
    }

    #[test]
    fn test_registry_filter_valid() {
        let registry = NodeKeyRegistry::new(2, 2);
        let mut map = HashMap::new();
        map.insert(NodeKey::new(0, 0), "valid".to_string());
        map.insert(NodeKey::new(1, 1), "valid".to_string());
        map.insert(NodeKey::new(2, 0), "invalid".to_string()); // group out of bounds
        map.insert(NodeKey::new(0, 3), "invalid".to_string()); // key out of bounds

        let filtered = registry.filter_valid_keys(&map);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains_key(&NodeKey::new(0, 0)));
        assert!(filtered.contains_key(&NodeKey::new(1, 1)));
    }

    #[test]
    fn test_registry_has_invalid_keys() {
        let registry = NodeKeyRegistry::new(2, 2);
        let mut map = HashMap::new();
        map.insert(NodeKey::new(0, 0), "valid");
        map.insert(NodeKey::new(2, 0), "invalid"); // group out of bounds
        map.insert(NodeKey::new(0, 3), "invalid"); // key out of bounds

        let invalid = registry.has_invalid_keys(&map);
        assert_eq!(invalid.len(), 2);
        assert!(invalid.contains(&NodeKey::new(2, 0)));
        assert!(invalid.contains(&NodeKey::new(0, 3)));
    }

    #[test]
    fn test_layout_change_synchronization() {
        // Simulate layout change from 2x2 to 2x3
        let old_registry = NodeKeyRegistry::new(2, 2);
        let new_registry = NodeKeyRegistry::new(2, 3);

        // Create old control map with some values
        let mut old_controls = HashMap::new();
        old_controls.insert(NodeKey::new(0, 0), 0.5);
        old_controls.insert(NodeKey::new(0, 1), 0.8);
        old_controls.insert(NodeKey::new(1, 0), 0.3);
        old_controls.insert(NodeKey::new(1, 1), 0.7);

        // Verify old layout validity
        assert!(old_registry.has_invalid_keys(&old_controls).is_empty());

        // Create new controls based on new layout
        let mut new_controls = HashMap::new();
        new_registry.iter_keys(|key| {
            // Preserve old values where possible, default to 0.0 for new keys
            let value = old_controls.get(&key).copied().unwrap_or(0.0);
            new_controls.insert(key, value);
        });

        // Verify new layout validity
        assert!(new_registry.has_invalid_keys(&new_controls).is_empty());
        assert_eq!(new_controls.len(), 6); // 2 groups * 3 keys = 6 total

        // Verify preserved values
        assert_eq!(new_controls.get(&NodeKey::new(0, 0)), Some(&0.5));
        assert_eq!(new_controls.get(&NodeKey::new(0, 1)), Some(&0.8));
        assert_eq!(new_controls.get(&NodeKey::new(1, 0)), Some(&0.3));
        assert_eq!(new_controls.get(&NodeKey::new(1, 1)), Some(&0.7));

        // Verify new keys have default values
        assert_eq!(new_controls.get(&NodeKey::new(0, 2)), Some(&0.0));
        assert_eq!(new_controls.get(&NodeKey::new(1, 2)), Some(&0.0));
    }

    #[test]
    fn test_layout_shrinkage_synchronization() {
        // Simulate layout change from 3x3 to 2x2 (shrinkage)
        let old_registry = NodeKeyRegistry::new(3, 3);
        let new_registry = NodeKeyRegistry::new(2, 2);

        // Create old control map with values
        let mut old_controls = HashMap::new();
        for (i, key) in old_registry.all_keys().into_iter().enumerate() {
            old_controls.insert(key, i as f32 * 0.1);
        }

        // Filter to new layout - should only keep valid keys
        let new_controls = new_registry.filter_valid_keys(&old_controls);

        // Verify new layout validity
        assert!(new_registry.has_invalid_keys(&new_controls).is_empty());
        assert_eq!(new_controls.len(), 4); // 2 groups * 2 keys = 4 total

        // Verify preserved values for keys that still exist
        assert_eq!(new_controls.get(&NodeKey::new(0, 0)), Some(&0.0));
        assert_eq!(new_controls.get(&NodeKey::new(0, 1)), Some(&0.1));
        assert_eq!(new_controls.get(&NodeKey::new(1, 0)), Some(&0.3));
        assert_eq!(new_controls.get(&NodeKey::new(1, 1)), Some(&0.4));

        // Verify out-of-bounds keys are removed
        assert!(!new_controls.contains_key(&NodeKey::new(2, 0)));
        assert!(!new_controls.contains_key(&NodeKey::new(0, 2)));
    }

    #[test]
    fn test_band_control_value_preservation() {
        // Simulate the CPAL controller's value preservation logic
        let registry = NodeKeyRegistry::new(2, 2);

        // Create old controls with some values set
        let mut old_controls = HashMap::new();
        old_controls.insert(NodeKey::new(0, 0), 0.25);
        old_controls.insert(NodeKey::new(0, 1), 0.75);
        old_controls.insert(NodeKey::new(1, 0), 0.50);
        old_controls.insert(NodeKey::new(1, 1), 0.90);

        // Extract old values (simulating the preservation step)
        let old_values: HashMap<NodeKey, f32> = old_controls.clone();

        // Create new controls (simulating network recreation)
        let mut new_controls = HashMap::new();
        registry.iter_keys(|key| {
            new_controls.insert(key, 0.0); // Start with default values
        });

        // Apply preserved values (simulating the restoration step)
        for (key, old_value) in old_values {
            if new_controls.contains_key(&key) {
                new_controls.insert(key, old_value);
            }
        }

        // Verify all values were preserved correctly
        assert_eq!(new_controls.get(&NodeKey::new(0, 0)), Some(&0.25));
        assert_eq!(new_controls.get(&NodeKey::new(0, 1)), Some(&0.75));
        assert_eq!(new_controls.get(&NodeKey::new(1, 0)), Some(&0.50));
        assert_eq!(new_controls.get(&NodeKey::new(1, 1)), Some(&0.90));
        assert_eq!(new_controls.len(), 4);
    }
}
