//! The PlayerPrefs analog, a human-readable JSON file. Reads resolve from memory loaded
//! once at a boundary ([`open`]); writes persist only at [`flush`] (Stop or quit), never
//! inside `FixedUpdate`, so disk contents never leak into a tick. A pathless store
//! ([`new`]) never touches disk.
//!
//! [`open`]: Storage::open
//! [`flush`]: Storage::flush
//! [`new`]: Storage::new
//!
//! stored-format: the PlayerPrefs save file

use std::path::PathBuf;

use serde_json::{Map, Value};

pub const DEFAULT_STORAGE_PATH: &str = "saved/storage.json";

#[derive(Default)]
pub struct Storage {
    path: Option<PathBuf>,
    data: Map<String, Value>,
}

impl Storage {
    /// Pathless, for the harness and tests, which must never read a developer's save.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self, path: impl Into<PathBuf>) -> Result<(), String> {
        let path = path.into();
        if path.exists() {
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read storage file: {}", e))?;
            let value: Value = serde_json::from_str(&text)
                .map_err(|e| format!("Failed to parse storage file: {}", e))?;
            self.data = match value {
                Value::Object(map) => map,
                _ => Map::new(),
            };
        }
        self.path = Some(path);
        Ok(())
    }

    pub fn flush(&self) -> Result<(), String> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let text = serde_json::to_string_pretty(&Value::Object(self.data.clone()))
            .map_err(|e| format!("Failed to serialize storage: {}", e))?;
        // lint: allow(asset-write) game save data, not read by a path-keyed cache
        crate::fs::write_atomic(path, text.as_bytes())
            .map_err(|e| format!("Failed to write storage file: {}", e))
    }

    pub fn data(&self) -> Value {
        Value::Object(self.data.clone())
    }

    pub fn replace(&mut self, data: Value) {
        self.data = match data {
            Value::Object(map) => map,
            _ => Map::new(),
        };
    }

    pub fn get(&self, namespace: &str, key: &str) -> Option<Value> {
        self.data.get(namespace)?.as_object()?.get(key).cloned()
    }

    pub fn set(&mut self, namespace: &str, key: &str, value: Value) {
        let entry = self
            .data
            .entry(namespace.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !entry.is_object() {
            *entry = Value::Object(Map::new());
        }
        entry
            .as_object_mut()
            .expect("namespace entry is an object")
            .insert(key.to_string(), value);
    }

    pub fn has(&self, namespace: &str, key: &str) -> bool {
        self.get(namespace, key).is_some()
    }

    pub fn delete(&mut self, namespace: &str, key: &str) -> bool {
        self.data
            .get_mut(namespace)
            .and_then(Value::as_object_mut)
            .and_then(|obj| obj.remove(key))
            .is_some()
    }

    pub fn get_namespace(&self, namespace: &str) -> Option<Value> {
        self.data.get(namespace).cloned()
    }

    pub fn set_namespace(&mut self, namespace: &str, value: Value) {
        self.data.insert(namespace.to_string(), value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_has_delete_in_memory() {
        let mut s = Storage::new();
        s.set("audio", "volume", 0.8.into());
        assert_eq!(s.get("audio", "volume"), Some(0.8.into()));
        assert!(s.has("audio", "volume"));
        assert!(s.delete("audio", "volume"));
        assert!(!s.has("audio", "volume"));
        assert!(s.flush().is_ok());
    }

    #[test]
    fn open_flush_roundtrips_through_a_file() {
        let path = rusty_test_support::temp::dir().join("rusty_storage_roundtrip.json");
        let _ = std::fs::remove_file(&path);

        let mut s = Storage::new();
        s.open(&path).unwrap();
        s.set("settings", "fullscreen", Value::Bool(true));
        s.set_namespace(
            "keybinds",
            serde_json::json!({ "jump": "Space", "fire": "Mouse0" }),
        );
        s.flush().unwrap();

        let mut loaded = Storage::new();
        loaded.open(&path).unwrap();
        assert_eq!(
            loaded.get("settings", "fullscreen"),
            Some(Value::Bool(true))
        );
        assert_eq!(
            loaded.get_namespace("keybinds"),
            Some(serde_json::json!({ "jump": "Space", "fire": "Mouse0" }))
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_file_is_not_an_error() {
        let path = rusty_test_support::temp::dir().join("rusty_storage_absent_xyz.json");
        let _ = std::fs::remove_file(&path);
        let mut s = Storage::new();
        assert!(s.open(&path).is_ok());
        assert!(s.get("any", "thing").is_none());
    }
}
