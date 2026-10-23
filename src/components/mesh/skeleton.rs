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

use super::MeshComponent;
use crate::components::{
    escape_token, EntityRefs, JointComponent, RefVisit, RigidBodyComponent, TransformComponent,
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BoneBinding {
    /// The bone entity of each joint slot, in the skin's joint order — what the
    /// palette build reads. Runtime only: bones get fresh ids every load, and an
    /// empty or stale list is re-bound by name (`Scene::sync_skeletons`). A
    /// destroyed bone's slot is `None`, **missing** (#1273): it skins at its bind
    /// pose until the skeleton is rebuilt, and never holds a dead id a later
    /// entity could be given.
    #[serde(skip)]
    pub bones: Vec<Option<u32>>,
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

    /// Every slot's bone, for a test of a whole skeleton: panics on a missing one.
    #[cfg(test)]
    pub fn ids(&self) -> Vec<u32> {
        self.bones
            .iter()
            .map(|b| b.expect("no slot is missing"))
            .collect()
    }
}

/// A bone's saved `Rigidbody` and `Joint`. Bones get fresh ids on every load, so
/// a joint to a bone of the same skeleton is saved as `connected_bone`, that bone's
/// chain (its name, then its ancestor bones', nearest first, like every bone
/// reference, #1277), with its id field cleared; one to another skeleton's bone by
/// `bone_refs`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoneBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rigidbody: Option<RigidBodyComponent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joint: Option<JointComponent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub connected_bone: Vec<String>,
    /// A joint to a bone of another skeleton (#1264): the joint names that skinned
    /// entity, and this the bone, as `Entity::bone_refs` does (keyed `/joint/…`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bone_refs: BTreeMap<String, Vec<String>>,
}

impl MeshComponent {
    /// Whether `pointer`, under the mesh, is a saved bone joint's id leaf:
    /// `/skeleton/bodies/<bone>/joint/connected_body`.
    pub fn is_ref_pointer(pointer: &str) -> bool {
        let parts: Vec<&str> = pointer.split('/').collect();
        matches!(
            parts.as_slice(),
            ["", "skeleton", "bodies", _, "joint", "connected_body"]
        )
    }
}

/// A saved ragdoll joint names its connected body by id when that body is not a
/// bone of the same skeleton (that one is `connected_bone`): a prop, or another
/// skinned entity with the bone in `bone_refs`. It follows the walk (#1264) like any
/// joint; the live bones' own joints are walked on the bones.
impl EntityRefs for MeshComponent {
    fn visit_refs(&mut self, visit: &mut RefVisit<'_>) {
        for (name, body) in &mut self.skeleton.bodies {
            if let Some(joint) = &mut body.joint {
                let pointer = format!("/skeleton/bodies/{}/joint", escape_token(name));
                joint.visit_refs(&mut |p, r| visit(&format!("{pointer}{p}"), r));
            }
        }
    }

    fn each_ref(&self, visit: &mut dyn FnMut(&str, u32)) {
        for (name, body) in &self.skeleton.bodies {
            if let Some(joint) = &body.joint {
                let pointer = format!("/skeleton/bodies/{}/joint", escape_token(name));
                joint.each_ref(&mut |p, r| visit(&format!("{pointer}{p}"), r));
            }
        }
    }

    /// Also marks a bone slot naming a destroyed entity missing (#1273): the slots
    /// join this one clearing pass (a despawn, a load leaving the entity behind),
    /// so no path leaves a dead id a later entity could be given. They stay out of
    /// [`EntityRefs::visit_refs`], the saved-id walk: they are never saved.
    fn forget(&mut self, dead: &dyn Fn(u32) -> bool) {
        self.visit_refs(&mut |_, r| {
            if r.is_some_and(dead) {
                *r = None;
            }
        });
        for slot in &mut self.skeleton.bones {
            if slot.is_some_and(dead) {
                *slot = None;
            }
        }
    }

    fn names_any(&self, dead: &dyn Fn(u32) -> bool) -> bool {
        let mut hit = self.skeleton.bones.iter().flatten().any(|&b| dead(b));
        self.each_ref(&mut |_, r| hit |= dead(r));
        hit
    }

    #[cfg(test)]
    fn every_ref() -> Self {
        let body = BoneBody {
            rigidbody: None,
            joint: Some(JointComponent::default()),
            connected_bone: Vec::new(),
            bone_refs: BTreeMap::new(),
        };
        let mut mesh = Self::new("Box", Vec::new(), Vec::new());
        mesh.skeleton.bodies.insert("hand".into(), body);
        mesh
    }
}
