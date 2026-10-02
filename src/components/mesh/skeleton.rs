//! src/components/mesh/skeleton.rs — a skinned mesh's binding to its bone
//! GameObjects (#453), Unity's `SkinnedMeshRenderer.bones`.
//!
//! The skeleton is intrinsic to the imported asset, so the bones themselves are
//! rebuilt from the model on load and never saved (`scene::skeleton`). What the
//! scene does save is the designer's adjustment: a bone moved while setting the
//! scene up is recorded here as a per-bone **override**, keyed by bone name, the way
//! a Unity prefab override records a change against its source. A ragdoll's
//! per-bone `Rigidbody` and `Joint` (#466) are saved the same way, as
//! [`BoneBody`]s keyed by bone name, the joint's connected bone by name too.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::components::{JointComponent, RigidBodyComponent, TransformComponent};

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
    /// Bone name → the physics a bone carries (a ragdoll's body and joint, #466),
    /// saved and restored like `overrides`: filled on save, consumed on load.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bodies: BTreeMap<String, BoneBody>,
}

impl BoneBinding {
    /// Whether there is nothing to save: no override or bone body recorded.
    pub fn is_unmodified(&self) -> bool {
        self.overrides.is_empty() && self.bodies.is_empty()
    }
}

/// A bone's saved `Rigidbody` and `Joint`. Bones get fresh ids on every load, so
/// the joint's `connected_body` is saved as `connected_bone`, a bone name of the
/// same skeleton, and its id field is cleared.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoneBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rigidbody: Option<RigidBodyComponent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joint: Option<JointComponent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connected_bone: Option<String>,
}
