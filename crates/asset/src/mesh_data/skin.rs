use glam::{Mat4, Quat, Vec3};

/// Per-joint data in `skin.joints` order, which `joint_indices` reference.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SkinData {
    pub inverse_bind: Vec<Mat4>,
    pub bind_global: Vec<Mat4>,
    pub local_bind: Vec<JointTransform>,
    /// `None` for a skeleton root.
    pub parents: Vec<Option<usize>>,
    /// What animation channels target.
    pub joint_nodes: Vec<usize>,
    pub mesh_inverse: Mat4,
    /// `joint_{slot}` when unnamed. Bones re-bind by it.
    pub names: Vec<String>,
}

impl SkinData {
    pub fn name(&self, slot: usize) -> String {
        self.names
            .get(slot)
            .cloned()
            .unwrap_or_else(|| format!("joint_{slot}"))
    }

    /// An `Armature` node's rotation and scale between the skinned node and the
    /// root: `bind_global * local_bind⁻¹`. Identity for a non-root joint.
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

    /// Undoes an `Armature` above the mesh (Blender's Z-up turn, centimetre
    /// scale) so up is +Y. Read a rig's up or facing here, not mesh-local.
    pub fn mesh_to_model(&self) -> Mat4 {
        let m = self.mesh_inverse.inverse();
        if m.is_finite() {
            m
        } else {
            Mat4::IDENTITY
        }
    }

    /// `None` when the axes coincide, so a Y-up rig computes as without it.
    pub fn mesh_to_model_axes(&self) -> Option<Mat4> {
        let m = self.mesh_to_model();
        (glam::Mat3::from_mat4(m) != glam::Mat3::IDENTITY).then_some(m)
    }

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
