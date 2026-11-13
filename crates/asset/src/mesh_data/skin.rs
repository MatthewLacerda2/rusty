//! crates/asset/src/mesh_data/skin.rs — a sub-mesh's skeleton ([`SkinData`]) and a
//! joint's local transform ([`JointTransform`]).

use glam::{Mat4, Quat, Vec3};

/// A sub-mesh's skeleton, parsed from a glTF `skin`: one inverse-bind matrix and
/// one bind-pose global (model-space) transform per joint, in `skin.joints` order
/// — the same order the per-vertex `joint_indices` reference.
///
/// At bind pose the skinning matrix for each joint is `bind_global * inverse_bind`,
/// which is the identity by construction (the inverse-bind is the inverse of the
/// joint's global bind transform). Storing both halves rather than only the product
/// keeps the rig data the animation runtime (#80) will pose, while letting the
/// renderer upload the bind-pose palette today (#79).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SkinData {
    /// Inverse bind matrix per joint.
    pub inverse_bind: Vec<Mat4>,
    /// Global (model-space) transform of each joint node at bind time, already
    /// expressed relative to the skinned mesh node so the palette is mesh-local.
    pub bind_global: Vec<Mat4>,
    /// Local bind-pose transform (TRS) of each joint node, in joint order. This is
    /// the rest pose the animation runtime (#80) overrides per channel before
    /// re-composing the joint globals; untouched channels keep these defaults.
    pub local_bind: Vec<JointTransform>,
    /// Parent joint *slot* of each joint (the index into this skin's joint arrays),
    /// or `None` for a joint whose parent is not itself a joint (a skeleton root).
    /// Drives the hierarchy composition when posing.
    pub parents: Vec<Option<usize>>,
    /// glTF node index of each joint, in joint order — the key animation channels
    /// target. Lets the clip importer map a channel's node onto a joint slot.
    pub joint_nodes: Vec<usize>,
    /// Inverse of the skinned mesh node's global bind transform. Left-multiplying a
    /// posed joint global by this re-expresses the palette in mesh-local space, the
    /// same convention `bind_global` already bakes in.
    pub mesh_inverse: Mat4,
    /// Name of each joint node, in joint order (`joint_{slot}` when the glTF node
    /// is unnamed). The key a bone GameObject is spawned under and re-bound by
    /// (#453), so overrides and attachments survive a re-exported skeleton.
    pub names: Vec<String>,
}

impl SkinData {
    /// The joint's name, or the `joint_{slot}` fallback for a skin built without
    /// names.
    pub fn name(&self, slot: usize) -> String {
        self.names
            .get(slot)
            .cloned()
            .unwrap_or_else(|| format!("joint_{slot}"))
    }

    /// The fixed transform from mesh-local space into a root joint's parent space:
    /// whatever sits between the skinned node and the skeleton root in the glTF (an
    /// `Armature` node's rotation and scale). Recovered as
    /// `bind_global * local_bind⁻¹`, so a root bone's mesh-local transform is this
    /// times its sampled local. Identity for a joint that has a parent joint.
    pub fn root_offset(&self, slot: usize) -> Mat4 {
        if self.parents.get(slot).copied().flatten().is_some() {
            return Mat4::IDENTITY;
        }
        let (Some(global), Some(local)) = (self.bind_global.get(slot), self.local_bind.get(slot))
        else {
            return Mat4::IDENTITY;
        };
        *global * local.matrix().inverse()
    }

    /// From mesh-local space (where `bind_global` lives) into model space: the
    /// skinned mesh node's own transform, so an `Armature` node above it (a
    /// Blender export's Z-up turn and centimetre scale) is undone and up is +Y,
    /// as glTF has it. What reads a rig's up or facing reads it here, never in
    /// mesh-local space; the identity when `mesh_inverse` can't be inverted.
    pub fn mesh_to_model(&self) -> Mat4 {
        let m = self.mesh_inverse.inverse();
        if m.is_finite() {
            m
        } else {
            Mat4::IDENTITY
        }
    }

    /// [`mesh_to_model`](Self::mesh_to_model) when it turns or scales, `None`
    /// when mesh-local and model axes coincide (at most a translation between
    /// them, which moves no direction or displacement). What reads directions
    /// takes the plain path on `None`, so a native Y-up rig computes exactly as
    /// it would without the conversion.
    pub fn mesh_to_model_axes(&self) -> Option<Mat4> {
        let m = self.mesh_to_model();
        (glam::Mat3::from_mat4(m) != glam::Mat3::IDENTITY).then_some(m)
    }

    /// The bind-pose bone palette: `bind_global[j] * inverse_bind[j]` per joint.
    /// Joints are paired in order; a length mismatch is truncated to the shorter.
    pub fn bind_palette(&self) -> Vec<Mat4> {
        self.bind_global
            .iter()
            .zip(self.inverse_bind.iter())
            .map(|(g, ib)| *g * *ib)
            .collect()
    }
}

/// A joint's local transform as decomposed translation / rotation / scale — the
/// form glTF animation channels address (separate T/R/S paths) and the form the
/// keyframe sampler interpolates. Composes to a `Mat4` via [`JointTransform::matrix`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JointTransform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for JointTransform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl JointTransform {
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}
