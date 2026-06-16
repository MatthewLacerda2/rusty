//! src/scene/layers.rs — the project-wide Layers registry.
//!
//! One shared, Unity-style **Layers** list per World (issue #90). Every entity
//! carries a single `layer: u8` (an index into this registry); the same 32 slots
//! are what the physics collision matrix (#91) and the camera culling masks (#92)
//! will reference. This is **groundwork only** — the registry stores names, not
//! behaviour: nothing here changes what collides or what renders.
//!
//! Layer 0 is always `"Default"` and cannot be renamed or cleared, mirroring
//! Unity's fixed first slot. Names serialize with the project (on `SceneData`) so
//! they survive save/load. An empty slot is "unnamed" and displays as `Layer N`.

use serde::{Deserialize, Serialize};

/// Number of layers, Unity-identical. A layer *index* fits in a `u8`; a layer
/// *membership set* is a `u32` bitmask (one bit per layer) — the form #91/#92 use.
pub const LAYER_COUNT: usize = 32;

/// The reserved name of layer 0, which is fixed.
pub const DEFAULT_LAYER_NAME: &str = "Default";

/// The project's named layers: exactly [`LAYER_COUNT`] slots indexed by layer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayerRegistry {
    /// One name per slot. Index 0 is always `"Default"`; an empty string marks an
    /// unnamed user slot. Kept at exactly [`LAYER_COUNT`] entries by [`normalize`].
    ///
    /// [`normalize`]: LayerRegistry::normalize
    names: Vec<String>,
}

impl Default for LayerRegistry {
    fn default() -> Self {
        let mut names = vec![String::new(); LAYER_COUNT];
        names[0] = DEFAULT_LAYER_NAME.to_string();
        Self { names }
    }
}

impl LayerRegistry {
    /// The raw name of `index` (empty for an unnamed slot, or out of range).
    pub fn name(&self, index: u8) -> &str {
        self.names
            .get(index as usize)
            .map(String::as_str)
            .unwrap_or("")
    }

    /// A display label: the slot's name, or `Layer N` when it is unnamed.
    pub fn label(&self, index: u8) -> String {
        let name = self.name(index);
        if name.is_empty() {
            format!("Layer {index}")
        } else {
            name.to_string()
        }
    }

    /// Rename a layer. Layer 0 (`"Default"`) is fixed — the call is a no-op for it
    /// and for any out-of-range index. Names are trimmed; a blank name clears the
    /// slot back to unnamed.
    pub fn set_name(&mut self, index: u8, name: impl Into<String>) {
        if index == 0 {
            return;
        }
        if let Some(slot) = self.names.get_mut(index as usize) {
            *slot = name.into().trim().to_string();
        }
    }

    /// Look up a layer by name (Unity's `LayerMask.NameToLayer`). Returns `None`
    /// for an unknown or empty name.
    pub fn index_of(&self, name: &str) -> Option<u8> {
        if name.is_empty() {
            return None;
        }
        self.names.iter().position(|n| n == name).map(|i| i as u8)
    }

    /// Iterate `(index, name)` over every slot, including unnamed ones.
    pub fn iter(&self) -> impl Iterator<Item = (u8, &str)> {
        self.names
            .iter()
            .enumerate()
            .map(|(i, n)| (i as u8, n.as_str()))
    }

    /// Coerce a freshly-deserialized registry back to exactly [`LAYER_COUNT`]
    /// slots with a fixed `"Default"` at index 0, so a hand-edited or legacy file
    /// can never leave the registry malformed.
    pub fn normalize(&mut self) {
        self.names.resize(LAYER_COUNT, String::new());
        self.names[0] = DEFAULT_LAYER_NAME.to_string();
    }
}
