//! src/core/storage.rs — Storage: JSON-backed key-value persistence.
//!
//! The engine's PlayerPrefs analog (issue #86): a per-World key-value store
//! developers (and the engine's own settings) save across runs, backed by a
//! human-readable JSON file so an agent can read/diff a save in a PR. Values are
//! grouped into **namespaces** (one JSON object each), and a value can be any
//! structured blob, not just a flat scalar.
//!
//! ## Determinism discipline
//! The simulation is a pure function of `(seed, inputs, fixed dt)`, so the store
//! must never let disk contents leak into a tick:
//! - **Reads** resolve from the in-memory map, loaded once at a boundary
//!   ([`open`]) — the loaded snapshot *is* an input, like the scene itself.
//! - **Writes** mutate the in-memory map and are persisted only by [`flush`] at a
//!   boundary (Stop / quit), never inside `FixedUpdate`.
//! - A pathless store ([`new`]) never touches disk, so the headless harness and
//!   tests stay reproducible — `flush` is a no-op without a path.
//!
//! `core` is outside the determinism guard's sim dirs and stays decoupled from
//! egui / wgpu / mlua; the Lua `Storage` namespace lives in `api/storage.rs`.
//!
//! [`open`]: Storage::open
//! [`flush`]: Storage::flush
//! [`new`]: Storage::new

use std::path::PathBuf;

use serde_json::{Map, Value};

/// Default on-disk location for the windowed app's store (the gitignored runtime
/// workspace, alongside the seeded scenes). The harness/tests leave the store
/// pathless instead.
pub const DEFAULT_STORAGE_PATH: &str = "project/storage.json";

/// A namespaced, JSON-backed key-value store. The top level maps a namespace name
/// to a JSON object of its keys.
#[derive(Default)]
pub struct Storage {
    /// `Some` once bound to a file via [`Storage::open`]; `None` keeps the store
    /// entirely in memory (harness/tests), so [`Storage::flush`] never writes.
    path: Option<PathBuf>,
    /// namespace -> object of `{ key: value }`.
    data: Map<String, Value>,
}

impl Storage {
    /// An empty, pathless store — the default for the harness and tests, which
    /// must never read a developer's real save.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind the store to `path` and load it into memory if the file exists. This
    /// is the boundary read; a missing file is the normal first-run case (not an
    /// error). Returns `Err` only when an existing file is unreadable or malformed.
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

    /// Persist the in-memory store to disk as pretty JSON. A no-op when the store
    /// is pathless. The boundary write — called on Stop / quit, never per tick.
    pub fn flush(&self) -> Result<(), String> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let text = serde_json::to_string_pretty(&Value::Object(self.data.clone()))
            .map_err(|e| format!("Failed to serialize storage: {}", e))?;
        std::fs::write(path, text).map_err(|e| format!("Failed to write storage file: {}", e))
    }

    /// The value at `namespace.key`, or `None` if either is absent.
    pub fn get(&self, namespace: &str, key: &str) -> Option<Value> {
        self.data.get(namespace)?.as_object()?.get(key).cloned()
    }

    /// Set `namespace.key` to `value`, creating the namespace object if needed.
    pub fn set(&mut self, namespace: &str, key: &str, value: Value) {
        let entry = self
            .data
            .entry(namespace.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !entry.is_object() {
            *entry = Value::Object(Map::new());
        }
        // Safe: just ensured `entry` is an object.
        entry
            .as_object_mut()
            .expect("namespace entry is an object")
            .insert(key.to_string(), value);
    }

    /// Whether `namespace.key` exists.
    pub fn has(&self, namespace: &str, key: &str) -> bool {
        self.get(namespace, key).is_some()
    }

    /// Remove `namespace.key`. Returns whether something was removed.
    pub fn delete(&mut self, namespace: &str, key: &str) -> bool {
        self.data
            .get_mut(namespace)
            .and_then(Value::as_object_mut)
            .and_then(|obj| obj.remove(key))
            .is_some()
    }

    /// The whole namespace object as a structured blob, or `None` if absent.
    pub fn get_namespace(&self, namespace: &str) -> Option<Value> {
        self.data.get(namespace).cloned()
    }

    /// Replace (or create) a whole namespace with `value` in one shot.
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
        // Pathless flush is a harmless no-op.
        assert!(s.flush().is_ok());
    }

    #[test]
    fn open_flush_roundtrips_through_a_file() {
        let path = std::env::temp_dir().join("rusty_storage_roundtrip.json");
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
        let path = std::env::temp_dir().join("rusty_storage_absent_xyz.json");
        let _ = std::fs::remove_file(&path);
        let mut s = Storage::new();
        assert!(s.open(&path).is_ok());
        assert!(s.get("any", "thing").is_none());
    }
}
