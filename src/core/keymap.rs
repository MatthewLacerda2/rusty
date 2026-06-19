//! src/core/keymap.rs — Keymap: physical→logical key remap at the input source.
//!
//! Rebindable controls (issue #88). A [`Keymap`] is a table from a *physical* key
//! name (what the keyboard hardware reports, e.g. the QWERTY `W` key) to a
//! *logical* key name (what gameplay reads, e.g. the `"FORWARD"`-bound `"W"`). It
//! is applied **once, at the input source** — when winit physical-key events are
//! translated into [`InputState`](crate::core::input::InputState) — so the
//! simulation only ever sees logical keys and the sim-facing API
//! (`Input.IsKeyDown`) is unchanged.
//!
//! ## Where it lives in the determinism story
//! The remap sits in the platform layer (`main.rs`), strictly *before* the sim
//! reads a key. The headless harness and bot-players inject logical keys directly
//! via [`InputState::press`](crate::core::input::InputState::press)/[`release`](crate::core::input::InputState::release),
//! so they bypass the keymap entirely — the sim stays a pure function of
//! `(seed, inputs, fixed dt)`. `core` is outside the determinism guard's sim dirs.
//!
//! ## Persistence
//! A keymap round-trips through the [`Storage`](crate::core::storage::Storage)
//! layer as a flat JSON object of `{ physical: logical }` pairs. Only entries that
//! differ from identity are stored, so a default (unremapped) install persists an
//! empty object. Loaded at startup, editable at runtime.

use std::collections::HashMap;

use serde_json::{Map, Value};

/// The `Storage` namespace a keymap persists under.
pub const KEYBINDINGS_NAMESPACE: &str = "keybindings";
/// The key within [`KEYBINDINGS_NAMESPACE`] holding the `{ physical: logical }` map.
pub const KEYBINDINGS_KEY: &str = "bindings";

/// A physical→logical key remap. Absent entries pass through unchanged (identity),
/// so an empty `Keymap` is the default "no remap" state. Key names are normalized
/// to uppercase, matching [`InputState`](crate::core::input::InputState).
#[derive(Clone, Debug, Default)]
pub struct Keymap {
    /// Only non-identity overrides are stored; a physical key with no entry maps to
    /// itself.
    overrides: HashMap<String, String>,
}

impl Keymap {
    /// An empty identity keymap — every physical key maps to itself.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind `physical` to emit `logical` instead of itself. Binding a key to itself
    /// removes the override, keeping the table minimal.
    pub fn bind(&mut self, physical: &str, logical: &str) {
        let physical = physical.to_uppercase();
        let logical = logical.to_uppercase();
        if physical == logical {
            self.overrides.remove(&physical);
        } else {
            self.overrides.insert(physical, logical);
        }
    }

    /// Drop any override for `physical`, restoring identity for that key.
    pub fn unbind(&mut self, physical: &str) {
        self.overrides.remove(&physical.to_uppercase());
    }

    /// The logical key a physical key resolves to: its override if one exists,
    /// otherwise the physical key itself (identity).
    pub fn resolve(&self, physical: &str) -> String {
        let physical = physical.to_uppercase();
        self.overrides.get(&physical).cloned().unwrap_or(physical)
    }

    /// The explicit logical binding for `physical`, or `None` when it is identity.
    pub fn get(&self, physical: &str) -> Option<&str> {
        self.overrides
            .get(&physical.to_uppercase())
            .map(String::as_str)
    }

    /// Whether the keymap is pure identity (no overrides).
    pub fn is_identity(&self) -> bool {
        self.overrides.is_empty()
    }

    /// The overrides as a flat JSON object of `{ physical: logical }`, suitable for
    /// `Storage::set`. Identity (no overrides) serializes to an empty object.
    pub fn to_json(&self) -> Value {
        let mut map = Map::new();
        for (physical, logical) in &self.overrides {
            map.insert(physical.clone(), Value::String(logical.clone()));
        }
        Value::Object(map)
    }

    /// Build a keymap from a `{ physical: logical }` JSON object (as produced by
    /// [`Keymap::to_json`]). Non-string values and identity entries are ignored, so
    /// a malformed or partial blob degrades to identity rather than failing.
    pub fn from_json(value: &Value) -> Self {
        let mut keymap = Self::new();
        if let Value::Object(map) = value {
            for (physical, logical) in map {
                if let Value::String(logical) = logical {
                    keymap.bind(physical, logical);
                }
            }
        }
        keymap
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_passes_through_unmapped_keys() {
        let keymap = Keymap::new();
        assert!(keymap.is_identity());
        assert_eq!(keymap.resolve("W"), "W");
        assert_eq!(keymap.resolve("space"), "SPACE");
        assert!(keymap.get("W").is_none());
    }

    #[test]
    fn bind_remaps_and_normalizes_case() {
        let mut keymap = Keymap::new();
        keymap.bind("up", "w");
        assert_eq!(keymap.resolve("UP"), "W");
        assert_eq!(keymap.get("up"), Some("W"));
        assert!(!keymap.is_identity());
        // An untouched key still maps to itself.
        assert_eq!(keymap.resolve("D"), "D");
    }

    #[test]
    fn binding_a_key_to_itself_is_identity() {
        let mut keymap = Keymap::new();
        keymap.bind("W", "W");
        assert!(keymap.is_identity());
        keymap.bind("W", "A");
        keymap.bind("w", "w");
        assert!(keymap.is_identity());
    }

    #[test]
    fn unbind_restores_identity() {
        let mut keymap = Keymap::new();
        keymap.bind("UP", "W");
        keymap.unbind("up");
        assert_eq!(keymap.resolve("UP"), "UP");
    }

    #[test]
    fn json_roundtrips_only_overrides() {
        let mut keymap = Keymap::new();
        keymap.bind("UP", "W");
        keymap.bind("DOWN", "S");
        let json = keymap.to_json();

        let restored = Keymap::from_json(&json);
        assert_eq!(restored.resolve("UP"), "W");
        assert_eq!(restored.resolve("DOWN"), "S");
        assert_eq!(restored.resolve("LEFT"), "LEFT");
    }

    #[test]
    fn identity_serializes_to_empty_object() {
        assert_eq!(Keymap::new().to_json(), serde_json::json!({}));
    }

    #[test]
    fn from_json_ignores_malformed_entries() {
        let value = serde_json::json!({ "UP": "W", "DOWN": 42, "X": "X" });
        let keymap = Keymap::from_json(&value);
        assert_eq!(keymap.resolve("UP"), "W");
        // Non-string value ignored -> identity.
        assert_eq!(keymap.resolve("DOWN"), "DOWN");
        // Identity entry ("X" -> "X") ignored.
        assert!(keymap.get("X").is_none());
    }
}
