use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::MeshComponent;
use crate::{
    escape_token, EntityRefs, JointComponent, RefVisit, RigidBodyComponent, TransformComponent,
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BoneBinding {
    /// Each joint's bone, in the skin's order; never saved, as bones get fresh ids each load. A
    /// destroyed bone's slot is `None` and skins at its bind pose.
    #[serde(skip)]
    pub bones: Vec<Option<u32>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    /// Filled from the live bones on save and consumed on load, so the live value is empty.
    pub overrides: BTreeMap<String, TransformComponent>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bodies: BTreeMap<String, BoneBody>,
}

impl BoneBinding {
    pub fn is_unmodified(&self) -> bool {
        self.overrides.is_empty() && self.bodies.is_empty()
    }

    /// A test hook, for other crates' tests: panics on a missing slot.
    #[doc(hidden)]
    pub fn ids(&self) -> Vec<u32> {
        self.bones
            .iter()
            .map(|b| b.expect("no slot is missing"))
            .collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoneBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rigidbody: Option<RigidBodyComponent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joint: Option<JointComponent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// A joint to this skeleton's bone, by its chain (name, then ancestors nearest first).
    pub connected_bone: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bone_refs: BTreeMap<String, Vec<String>>,
}

impl MeshComponent {
    pub fn is_ref_pointer(pointer: &str) -> bool {
        let parts: Vec<&str> = pointer.split('/').collect();
        matches!(
            parts.as_slice(),
            ["", "skeleton", "bodies", _, "joint", "connected_body"]
        )
    }
}

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

    /// Bone slots clear here too, so no dead id outlives a despawn; never saved, they skip
    /// `visit_refs`.
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

    fn each_held(&self, visit: &mut dyn FnMut(u32)) {
        self.skeleton.bones.iter().flatten().for_each(|&b| visit(b));
        self.each_ref(&mut |_, r| visit(r));
    }

    #[doc(hidden)]
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
