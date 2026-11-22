use glam::{Mat4, Vec3};

use rusty_scene::Camera;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CubemapFace {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

impl CubemapFace {
    pub const ALL: [CubemapFace; 6] = [
        CubemapFace::PosX,
        CubemapFace::NegX,
        CubemapFace::PosY,
        CubemapFace::NegY,
        CubemapFace::PosZ,
        CubemapFace::NegZ,
    ];

    fn yaw_pitch(self) -> (f32, f32) {
        match self {
            CubemapFace::PosX => (0.0, 0.0),
            CubemapFace::NegX => (180.0, 0.0),
            CubemapFace::PosY => (0.0, 90.0),
            CubemapFace::NegY => (0.0, -90.0),
            CubemapFace::PosZ => (90.0, 0.0),
            CubemapFace::NegZ => (-90.0, 0.0),
        }
    }

    /// The `texture_cube` layout's up, as `sky/prefilter.wgsl`'s `face_direction`; explicit, as a
    /// look-at along ±Y is degenerate.
    fn up(self) -> Vec3 {
        match self {
            CubemapFace::PosY => Vec3::NEG_Z,
            CubemapFace::NegY => Vec3::Z,
            _ => Vec3::Y,
        }
    }
}

/// Renders its face mirrored (a cube face is left-handed); readback flips it with `mirror_rows`,
/// as a mirrored projection would reverse the winding and cull every front face.
pub(crate) fn face_camera(position: Vec3, face: CubemapFace) -> Camera {
    let (yaw, pitch) = face.yaw_pitch();
    let mut camera = Camera::new(position, yaw, pitch);
    camera.fov = 90.0;
    camera.up = face.up();
    camera
}

/// The face camera's matrix with x negated, as the readback flip leaves it: every by-direction
/// reader of a capture maps through this.
pub(crate) fn face_view_projection(position: Vec3, face: CubemapFace) -> Mat4 {
    Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0))
        * face_camera(position, face).build_view_projection(1.0)
}

pub(super) fn mirror_rows<T>(face: &mut [T], resolution: u32) {
    for line in face.chunks_exact_mut(resolution as usize) {
        line.reverse();
    }
}
