//! src/render/ibl/faces.rs — the six faces of a captured cube and how each is
//! aimed and laid out (#243, #1615): the cameras that render them, the readback
//! flip that lays them out as a `texture_cube` samples them, and the matrices every
//! reader of a capture maps directions through.

use glam::{Mat4, Vec3};

use crate::scene::Camera;

/// The six faces of a cubemap, in the conventional +X,-X,+Y,-Y,+Z,-Z order. The
/// index into [`CubemapCapture::faces`](super::cubemap_capture::CubemapCapture::faces) is `face as usize`.
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
    /// All six faces in storage order.
    pub const ALL: [CubemapFace; 6] = [
        CubemapFace::PosX,
        CubemapFace::NegX,
        CubemapFace::PosY,
        CubemapFace::NegY,
        CubemapFace::PosZ,
        CubemapFace::NegZ,
    ];

    /// The (yaw, pitch) in degrees that aims [`Camera`] down this face's axis, so the
    /// `PosX` face looks toward +X, etc.: [`Camera::forward`] is
    /// `(cosY·cosP, sinP, sinY·cosP)`. An earlier table aimed each face down the
    /// opposite axis, tuned against a test room whose camera sat inside every wall
    /// and saw the far side of the one behind it (#243); with back faces culled
    /// (#1615) the room was rebuilt and the table aims where its names say.
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

    /// The face's up vector in the `texture_cube` layout (the D3D / Vulkan / wgpu
    /// one, `sky/prefilter.wgsl`'s `face_direction`): +Y for the four sides, -Z
    /// looking up and +Z looking down. Explicit, so the ±Y faces' basis is not the
    /// rounding of a look-at whose forward is parallel to +Y (#1615).
    fn up(self) -> Vec3 {
        match self {
            CubemapFace::PosY => Vec3::NEG_Z,
            CubemapFace::NegY => Vec3::Z,
            _ => Vec3::Y,
        }
    }
}

/// A [`Camera`] at `position` aimed down `face`'s axis, up [`CubemapFace::up`], with
/// a 90° vertical FOV: the exact lens that tiles six faces into a seamless cube.
///
/// A cube face's basis is left-handed (right = up × forward), and a camera's is
/// right-handed, so what this camera renders is the face mirrored left to right;
/// the capture flips each face's rows back at readback ([`mirror_rows`]), not
/// through a mirrored projection, which would reverse the winding and cull every
/// front face. [`face_view_projection`] is this camera's matrix with that flip.
pub(crate) fn face_camera(position: Vec3, face: CubemapFace) -> Camera {
    let (yaw, pitch) = face.yaw_pitch();
    let mut camera = Camera::new(position, yaw, pitch);
    camera.fov = 90.0;
    camera.up = face.up();
    camera
}

/// World → the stored face's clip space for a cube captured at `position`: the
/// face camera's view-projection with x negated, as the readback flip leaves it.
/// Every reader of a capture's texels by direction (the CPU and GPU prefilters, the
/// SH projection: `cube_sample::FaceProjections`) maps through this, so all of them
/// agree with the stored layout.
pub(crate) fn face_view_projection(position: Vec3, face: CubemapFace) -> Mat4 {
    Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0))
        * face_camera(position, face).build_view_projection(1.0)
}

/// Reverse each `resolution`-wide RGBA8 row of `face` in place (see [`face_camera`]).
pub(super) fn mirror_rows(face: &mut [u8], resolution: u32) {
    let row = resolution as usize * 4;
    for line in face.chunks_exact_mut(row) {
        let texels = line.len() / 4;
        for x in 0..texels / 2 {
            let (a, b) = (x * 4, (texels - 1 - x) * 4);
            for c in 0..4 {
                line.swap(a + c, b + c);
            }
        }
    }
}
