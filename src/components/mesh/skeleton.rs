//! src/components/mesh/skeleton.rs — a skinned mesh's binding to its bone
//! GameObjects (#453), Unity's `SkinnedMeshRenderer.bones`.
//!
//! The skeleton is intrinsic to the imported asset, so the bones themselves are
//! rebuilt from the model on load and never saved (`scene::skeleton`). What the
//! scene does save is the designer's adjustment: a bone moved while setting the
//! scene up is recorded here as a per-bone **override**, keyed by bone name, the way
//! a Unity prefab override records a change against its source.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::components::TransformComponent;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BoneBinding {
    /// The bone entity of each joint slot, in the skin's joint order — what the
    /// palette build reads. Runtime only: bones get fresh ids every load, and an
    /// empty or stale list is re-bound by name (`Scene::sync_skeletons`).
    #[serde(skip)]
    pub bones: Vec<u32>,
    /// Bone name → the local transform a designer gave it, applied when the
    /// skeleton is (re)built. Filled from the live bones on save and consumed on
    /// load, so the live value is empty and the bones themselves are authoritative.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overrides: BTreeMap<String, TransformComponent>,
}

impl BoneBinding {
    /// Whether there is nothing to save: no override recorded.
    pub fn is_unmodified(&self) -> bool {
        self.overrides.is_empty()
    }
}
