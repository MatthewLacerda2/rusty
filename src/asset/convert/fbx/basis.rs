//! The change of basis from an FBX file's axes to glTF's (+X right, +Y up, +Z
//! front), baked into every vertex and transform.
//!
//! ufbx can convert axes too, but only by rotating the top-level nodes, and the
//! engine places a model's meshes without their node transforms: a Z-up mesh
//! would lie on its back. So the converter conjugates instead, the way Blender's
//! glTF exporter does: a point becomes `C·p`, a transform `C·M·C⁻¹`, and every
//! world transform comes out in glTF's axes with the meshes upright in their own.

use glam::{DMat3, DMat4, DVec3};

/// The rotation (or, for a left-handed file, reflection) taking file axes to
/// glTF's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Basis {
    c: DMat4,
    mirrored: bool,
}

impl Basis {
    /// The basis for a file whose axes are `axes`. Unknown axes (a file that does
    /// not say) are read as glTF's own.
    pub fn new(axes: &ufbx::CoordinateAxes) -> Self {
        let (right, up, front) = (dir(axes.right), dir(axes.up), dir(axes.front));
        let file = DMat3::from_cols(right, up, front);
        if file.determinant().abs() < 0.5 {
            return Basis {
                c: DMat4::IDENTITY,
                mirrored: false,
            };
        }
        // `file` is a signed permutation, so its inverse is its transpose.
        let c = file.transpose();
        Basis {
            c: DMat4::from_mat3(c),
            mirrored: c.determinant() < 0.0,
        }
    }

    /// A point or direction in glTF's axes.
    pub fn vector(&self, v: ufbx::Vec3) -> [f32; 3] {
        self.c
            .transform_vector3(DVec3::new(v.x, v.y, v.z))
            .as_vec3()
            .to_array()
    }

    /// A transform in glTF's axes.
    pub fn matrix(&self, m: DMat4) -> DMat4 {
        self.c * m * self.c.transpose()
    }

    /// True when the file is left-handed: its triangles' winding must flip.
    pub fn mirrored(&self) -> bool {
        self.mirrored
    }
}

fn dir(axis: ufbx::CoordinateAxis) -> DVec3 {
    use ufbx::CoordinateAxis as A;
    match axis {
        A::PositiveX => DVec3::X,
        A::NegativeX => DVec3::NEG_X,
        A::PositiveY => DVec3::Y,
        A::NegativeY => DVec3::NEG_Y,
        A::PositiveZ => DVec3::Z,
        A::NegativeZ => DVec3::NEG_Z,
        A::Unknown => DVec3::ZERO,
    }
}

/// A ufbx affine matrix as a glam one.
pub fn to_dmat4(m: &ufbx::Matrix) -> DMat4 {
    DMat4::from_cols_array(&[
        m.m00, m.m10, m.m20, 0.0, m.m01, m.m11, m.m21, 0.0, m.m02, m.m12, m.m22, 0.0, m.m03, m.m13,
        m.m23, 1.0,
    ])
}

/// A ufbx transform (TRS) as a glam matrix.
pub fn trs(t: &ufbx::Transform) -> DMat4 {
    DMat4::from_scale_rotation_translation(
        DVec3::new(t.scale.x, t.scale.y, t.scale.z),
        glam::DQuat::from_xyzw(t.rotation.x, t.rotation.y, t.rotation.z, t.rotation.w),
        DVec3::new(t.translation.x, t.translation.y, t.translation.z),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ufbx::CoordinateAxis as A;

    fn axes(right: A, up: A, front: A) -> ufbx::CoordinateAxes {
        ufbx::CoordinateAxes { right, up, front }
    }

    #[test]
    fn z_up_becomes_y_up() {
        let basis = Basis::new(&axes(A::PositiveX, A::PositiveZ, A::NegativeY));
        let up = basis.vector(ufbx::Vec3 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        });
        assert_eq!(up, [0.0, 1.0, 0.0]);
        assert!(!basis.mirrored());
    }

    #[test]
    fn gltf_axes_and_unknown_axes_change_nothing() {
        for a in [
            axes(A::PositiveX, A::PositiveY, A::PositiveZ),
            axes(A::Unknown, A::Unknown, A::Unknown),
        ] {
            assert_eq!(Basis::new(&a).matrix(DMat4::IDENTITY), DMat4::IDENTITY);
        }
    }

    #[test]
    fn a_left_handed_file_is_mirrored() {
        let basis = Basis::new(&axes(A::PositiveX, A::PositiveY, A::NegativeZ));
        assert!(basis.mirrored());
    }
}
