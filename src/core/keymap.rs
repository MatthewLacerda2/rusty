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
//! A game writes its bindings through the [`Storage`](crate::core::storage::Storage)
//! layer as a flat `{ physical: logical }` table (Lua
//! `Storage.SetTable("keybindings", { bindings = { … } })`, `docs/api/Storage.md`);
//! only entries that differ from identity need storing. The shell reads it once, at
//! boot ([`Keymap::from_json`]), so a rebinding takes effect on the next launch.

use std::collections::BTreeMap;

use serde_json::Value;

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
    overrides: BTreeMap<String, String>,
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

    /// The logical key a physical key resolves to: its override if one exists,
    /// otherwise the physical key itself (identity).
    pub fn resolve(&self, physical: &str) -> String {
        let physical = physical.to_uppercase();
        self.overrides.get(&physical).cloned().unwrap_or(physical)
    }

    /// Build a keymap from the `{ physical: logical }` JSON object a script stored.
    /// Non-string values and identity entries are ignored, so a malformed or partial
    /// blob degrades to identity rather than failing.
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
    use serde_json::json;

    use super::*;

    #[test]
    fn identity_passes_through_unmapped_keys() {
        let keymap = Keymap::new();
        assert!(keymap.overrides.is_empty());
        assert_eq!(keymap.resolve("W"), "W");
        assert_eq!(keymap.resolve("space"), "SPACE");
    }

    #[test]
    fn a_stored_blob_remaps_and_normalizes_case() {
        // The table `Storage.SetTable("keybindings", { bindings = { up = "w" } })` stores.
        let keymap = Keymap::from_json(&json!({ "up": "w", "Down": "S" }));
        assert_eq!(keymap.resolve("UP"), "W");
        assert_eq!(keymap.resolve("down"), "S");
        // An untouched key still maps to itself.
        assert_eq!(keymap.resolve("LEFT"), "LEFT");
    }

    #[test]
    fn binding_a_key_to_itself_is_identity() {
        let mut keymap = Keymap::new();
        keymap.bind("W", "W");
        assert!(keymap.overrides.is_empty());
        keymap.bind("W", "A");
        keymap.bind("w", "w");
        assert!(keymap.overrides.is_empty());
    }

    #[test]
    fn a_malformed_blob_degrades_to_identity() {
        let keymap = Keymap::from_json(&json!({ "UP": "W", "DOWN": 42, "X": "x" }));
        assert_eq!(keymap.resolve("UP"), "W");
        // Non-string value ignored -> identity.
        assert_eq!(keymap.resolve("DOWN"), "DOWN");
        // Identity entry ("X" -> "x") stores nothing.
        assert_eq!(keymap.overrides.len(), 1);
        assert!(Keymap::from_json(&json!([1, 2])).overrides.is_empty());
    }
}
