//! Physical-to-logical key remap, applied once at the input source, so the sim only sees
//! logical keys; the harness and bots inject logical keys and bypass it. Persisted as a
//! flat `{ physical: logical }` table in `Storage`, read at boot.

use std::collections::BTreeMap;

use serde_json::Value;

pub const KEYBINDINGS_NAMESPACE: &str = "keybindings";
pub const KEYBINDINGS_KEY: &str = "bindings";

#[derive(Clone, Debug, Default)]
pub struct Keymap {
    overrides: BTreeMap<String, String>,
}

impl Keymap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bind(&mut self, physical: &str, logical: &str) {
        let physical = physical.to_uppercase();
        let logical = logical.to_uppercase();
        if physical == logical {
            self.overrides.remove(&physical);
        } else {
            self.overrides.insert(physical, logical);
        }
    }

    pub fn resolve(&self, physical: &str) -> String {
        let physical = physical.to_uppercase();
        self.overrides.get(&physical).cloned().unwrap_or(physical)
    }

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
        let keymap = Keymap::from_json(&json!({ "up": "w", "Down": "S" }));
        assert_eq!(keymap.resolve("UP"), "W");
        assert_eq!(keymap.resolve("down"), "S");
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
        assert_eq!(keymap.resolve("DOWN"), "DOWN");
        assert_eq!(keymap.overrides.len(), 1);
        assert!(Keymap::from_json(&json!([1, 2])).overrides.is_empty());
    }
}
