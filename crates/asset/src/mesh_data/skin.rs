use glam::{Mat4, Quat, Vec3};

/// Per-joint data in `skin.joints` order, which the per-vertex `joint_indices`
/// reference. At bind pose `bind_global * inverse_bind` is the identity; both
/// halves are kept so the animation runtime can pose the rig.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SkinData {
    pub inverse_bind: Vec<Mat4>,
    /// At bind time, relative to the skinned mesh node so the palette is mesh-local.
    pub bind_global: Vec<Mat4>,
    /// The rest pose; channels the animation does not touch keep these.
    pub local_bind: Vec<JointTransform>,
    /// Parent joint slot, or `None` for a skeleton root.
    pub parents: Vec<Option<usize>>,
    /// glTF node index of each joint: what animation channels target.
    pub joint_nodes: Vec<usize>,
    /// Inverse of the skinned mesh node's global bind transform: left-multiplying a posed joint global by it gives mesh-local space.
    pub mesh_inverse: Mat4,
    /// `joint_{slot}` when unnamed. The key a bone GameObject is spawned and
    /// re-bound under, so overrides survive a re-exported skeleton.
    pub names: Vec<String>,
}

impl SkinData {
    pub fn name(&self, slot: usize) -> String {
        self.names
            .get(slot)
            .cloned()
            .unwrap_or_else(|| format!("joint_{slot}"))
    }

    /// What sits between the skinned node and the skeleton root (an `Armature`
    /// node's rotation and scale): `bind_global * local_bind⁻¹`. Identity for a
    /// joint that has a parent joint.
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

    /// Undoes an `Armature` node above the mesh (a Blender export's Z-up turn and
    /// centimetre scale) so up is +Y. Read a rig's up or facing here, never in
    /// mesh-local space. Identity when `mesh_inverse` can't be inverted.
    pub fn mesh_to_model(&self) -> Mat4 {
        let m = self.mesh_inverse.inverse();
        if m.is_finite() {
            m
        } else {
            Mat4::IDENTITY
        }
    }

    /// `None` when the axes coincide (at most a translation), so a native Y-up
    /// rig computes exactly as it would without the conversion.
    pub fn mesh_to_model_axes(&self) -> Option<Mat4> {
        let m = self.mesh_to_model();
        (glam::Mat3::from_mat4(m) != glam::Mat3::IDENTITY).then_some(m)
    }

    /// A length mismatch is truncated to the shorter.
    pub fn bind_palette(&self) -> Vec<Mat4> {
        self.bind_global
            .iter()
            .zip(self.inverse_bind.iter())
            .map(|(g, ib)| *g * *ib)
            .collect()
    }
}

/// Decomposed T/R/S: the form glTF animation channels address.
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
