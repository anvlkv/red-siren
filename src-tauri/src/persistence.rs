use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use common::error::{AppError, Result};

/// Persistence helpers for JSON load/save via tauri-plugin-store
///
/// Why:
/// - Centralize store access patterns across modules.
/// - Provide consistent error mapping and messages.
/// - Reduce duplication of (open -> get/set -> serialize/deserialize -> save) logic.
#[allow(clippy::module_inception)]
pub mod persistence {
    use super::*;

    /// Load a JSON value from a store key and deserialize to T.
    ///
    /// Returns:
    /// - Ok(Some(T)) when the key exists and deserializes successfully.
    /// - Ok(None) when the key does not exist in the store.
    /// - Err(AppError::Tauri) on store access failures.
    /// - Err(AppError::Internal) on deserialization failures.
    pub fn load_json<T>(app: &AppHandle, store_name: &str, key: &str) -> Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        let store = app
            .store(store_name)
            .map_err(|e| AppError::Tauri(format!("Plugin store error: [{e}]")))?;

        let opt: Option<Value> = store.get(key);

        match opt {
            None => Ok(None),
            Some(v) => {
                let parsed = serde_json::from_value::<T>(v).map_err(|e| AppError::Internal {
                    message: format!(
                        "Deserialization error for store `{}` key `{}`: [{e}]",
                        store_name, key
                    ),
                })?;
                Ok(Some(parsed))
            }
        }
    }

    /// Load a JSON value from a store key, falling back to T::default when missing.
    ///
    /// Returns:
    /// - Ok(T) where T is the deserialized value, or default if key is missing.
    /// - Err(AppError::Tauri) on store access failures.
    /// - Err(AppError::Internal) on deserialization failures.
    pub fn load_json_or_default<T>(app: &AppHandle, store_name: &str, key: &str) -> Result<T>
    where
        T: DeserializeOwned + Default,
    {
        match load_json::<T>(app, store_name, key) {
            Ok(Some(v)) => Ok(v),
            Ok(None) => Ok(T::default()),
            Err(e) => {
                log::error!(
                    "Store `{}` key `{}`: falling back to default due to load/deserialization error: {}",
                    store_name,
                    key,
                    e
                );
                Ok(T::default())
            }
        }
    }

    /// Save a serializable value under a store key and persist to disk.
    ///
    /// Returns:
    /// - Ok(()) on success.
    /// - Err(AppError::Tauri) on store access or save failures.
    /// - Err(AppError::Internal) on serialization failures.
    pub fn save_json<T>(app: &AppHandle, store_name: &str, key: &str, value: &T) -> Result<()>
    where
        T: Serialize,
    {
        let store = app
            .store(store_name)
            .map_err(|e| AppError::Tauri(format!("Plugin store error: [{e}]")))?;

        let val = serde_json::to_value(value).map_err(|e| AppError::Internal {
            message: format!(
                "Serialization error for store `{}` key `{}`: [{e}]",
                store_name, key
            ),
        })?;

        store.set(key, val);

        store
            .save()
            .map_err(|e| AppError::Tauri(format!("Plugin store error: [{e}]")))?;

        Ok(())
    }

    /// Save a primitive JSON bool under a store key and persist to disk.
    ///
    /// Convenience wrapper for `save_json`.
    pub fn save_bool(app: &AppHandle, store_name: &str, key: &str, value: bool) -> Result<()> {
        save_json(app, store_name, key, &value)
    }

    /// Load a JSON bool from a store key.
    ///
    /// Returns:
    /// - Ok(Some(bool)) when present.
    /// - Ok(None) when missing.
    /// - Err on store access or deserialization failures.
    pub fn load_bool(app: &AppHandle, store_name: &str, key: &str) -> Result<Option<bool>> {
        load_json::<bool>(app, store_name, key)
    }
}
