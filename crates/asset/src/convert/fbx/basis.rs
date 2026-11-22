//! Conjugates instead of ufbx's rotation of the top-level nodes, as Blender's glTF
//! exporter does (`C·p`, `C·M·C⁻¹`): roots get no synthetic rotation and a mesh
//! spawned alone by `file::sub` is upright.

use glam::{DMat3, DMat4, DQuat, DVec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Local {
    pub translation: [f32; 3],
    /// Unit quaternion `[x, y, z, w]`.
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Basis {
    c: DMat4,
    mirrored: bool,
}

impl Basis {
    /// Unknown axes are read as glTF's own.
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

    pub fn vector(&self, v: ufbx::Vec3) -> [f32; 3] {
        self.c
            .transform_vector3(DVec3::new(v.x, v.y, v.z))
            .as_vec3()
            .to_array()
    }

    pub fn rotation(&self, q: ufbx::Quat) -> [f32; 4] {
        let r = DMat3::from_quat(DQuat::from_xyzw(q.x, q.y, q.z, q.w));
        let c = DMat3::from_mat4(self.c);
        DQuat::from_mat3(&(c * r * c.transpose()))
            .normalize()
            .as_quat()
            .to_array()
    }

    /// Signs kept, as Unity's importer does.
    pub fn scale(&self, s: ufbx::Vec3) -> [f32; 3] {
        let c = DMat3::from_mat4(self.c);
        let m = c * DMat3::from_diagonal(DVec3::new(s.x, s.y, s.z)) * c.transpose();
        [m.x_axis.x, m.y_axis.y, m.z_axis.z].map(|v| v as f32)
    }

    /// Shared by rest poses and keys: splitting the conjugated matrix instead
    /// moves a negative scale's sign onto any axis and turns the rotation 180°,
    /// flattening a bone blended from rest to key 0.
    pub fn local(&self, t: &ufbx::Transform) -> Local {
        Local {
            translation: self.vector(t.translation),
            rotation: self.rotation(t.rotation),
            scale: self.scale(t.scale),
        }
    }

    pub fn matrix(&self, m: DMat4) -> DMat4 {
        self.c * m * self.c.transpose()
    }

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

pub fn to_dmat4(m: &ufbx::Matrix) -> DMat4 {
    DMat4::from_cols_array(&[
        m.m00, m.m10, m.m20, 0.0, m.m01, m.m11, m.m21, 0.0, m.m02, m.m12, m.m22, 0.0, m.m03, m.m13,
        m.m23, 1.0,
    ])
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

    fn keys_agree_with_the_conjugated_matrix(basis: Basis) {
        let q = DQuat::from_rotation_z(0.7) * DQuat::from_rotation_x(0.3);
        let s = DVec3::new(1.0, 2.0, 3.0);
        let fbx_q = ufbx::Quat {
            x: q.x,
            y: q.y,
            z: q.z,
            w: q.w,
        };
        let fbx_s = ufbx::Vec3 {
            x: s.x,
            y: s.y,
            z: s.z,
        };
        let m = basis.matrix(DMat4::from_scale_rotation_translation(s, q, DVec3::ZERO));
        let (scale, rotation, _) = m.to_scale_rotation_translation();
        assert!(glam::Vec3::from(basis.scale(fbx_s)).abs_diff_eq(scale.as_vec3(), 1e-5));
        let keyed = glam::Quat::from_array(basis.rotation(fbx_q));
        assert!(keyed.dot(rotation.as_quat()).abs() > 1.0 - 1e-5);
    }

    #[test]
    fn z_up_keys_agree_with_the_conjugated_matrix() {
        keys_agree_with_the_conjugated_matrix(Basis::new(&axes(
            A::PositiveX,
            A::PositiveZ,
            A::NegativeY,
        )));
    }

    #[test]
    fn mirrored_keys_agree_with_the_conjugated_matrix() {
        let basis = Basis::new(&axes(A::PositiveX, A::PositiveZ, A::PositiveY));
        assert!(basis.mirrored());
        keys_agree_with_the_conjugated_matrix(basis);
    }

    #[test]
    fn local_keeps_a_negative_scale_under_a_mirrored_basis() {
        let basis = Basis::new(&axes(A::PositiveX, A::PositiveZ, A::PositiveY));
        assert!(basis.mirrored());
        let (s, q, t) = (
            DVec3::new(1.0, -1.0, 2.0),
            DQuat::from_rotation_z(0.5),
            DVec3::new(1.0, 2.0, 3.0),
        );
        let local = basis.local(&ufbx::Transform {
            translation: ufbx::Vec3 {
                x: t.x,
                y: t.y,
                z: t.z,
            },
            rotation: ufbx::Quat {
                x: q.x,
                y: q.y,
                z: q.z,
                w: q.w,
            },
            scale: ufbx::Vec3 {
                x: s.x,
                y: s.y,
                z: s.z,
            },
        });
        let scale = glam::Vec3::from(local.scale);
        assert!(
            scale.abs_diff_eq(glam::Vec3::new(1.0, 2.0, -1.0), 1e-6),
            "{scale}"
        );
        let composed = glam::Mat4::from_scale_rotation_translation(
            scale,
            glam::Quat::from_array(local.rotation),
            glam::Vec3::from(local.translation),
        );
        let conjugated = basis.matrix(DMat4::from_scale_rotation_translation(s, q, t));
        assert!(
            composed.abs_diff_eq(conjugated.as_mat4(), 1e-5),
            "{composed}"
        );
    }

    #[test]
    fn a_left_handed_file_is_mirrored() {
        let basis = Basis::new(&axes(A::PositiveX, A::PositiveY, A::NegativeZ));
        assert!(basis.mirrored());
    }
}
