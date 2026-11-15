//! crates/preview/src/stand_in.rs — the stand-in a texture, shader or material is shown
//! on: which mesh, how many repeats of the material it carries, and the fixed camera
//! and light each mesh is judged under.
//!
//! The sphere, cube and Suzanne are a prop's view: one three-quarter key light, the
//! default orbit. The **plane** is a tiling surface's view, Unity's quad preview tilted
//! to the angle a wall or floor is judged at: a flat square seen about 25° above its
//! surface, under a light raking across it about 20° up, so a stain that comes back
//! every tile and a normal map's relief both read. Both framings are fixed, so two
//! captures of one mesh compare pixel for pixel.

use glam::{Quat, Vec3};

use rusty_components::{MaterialAsset, UvMapping};

use crate::{OrbitState, PreviewMesh, PreviewSubject};

/// The plane's edge, in metres: square, centred on [`PLANE_CENTRE`].
pub const PLANE_SIZE: f32 = 4.0;
/// Where the plane's centre sits: pushed away from the camera, so the orbit's target
/// (the world origin, on the plane) is nearer than the centre and the plane fills the
/// bottom of the frame with its far edge high in it.
pub const PLANE_CENTRE: Vec3 = Vec3::new(0.0, 0.0, -0.9);
/// The camera's elevation above the plane's surface, in degrees.
pub const PLANE_VIEW_ELEVATION: f32 = 25.0;
/// The raking light's elevation above the plane's surface, in degrees.
pub const PLANE_LIGHT_ELEVATION: f32 = 20.0;

/// The plane's camera: looking down -Z (the plane's V axis, so tile rows read as
/// rows), [`PLANE_VIEW_ELEVATION`] above it, near enough that the far corners meet the
/// frame's sides.
const PLANE_ORBIT: OrbitState = OrbitState {
    yaw: -90.0,
    pitch: -PLANE_VIEW_ELEVATION,
    distance: 2.0,
};

/// The width one repeat of `repeats = 1` spans on `mesh`, in metres: the sphere's
/// diameter, the cube's edge and Suzanne's fitted size are all about one metre.
pub fn extent(mesh: PreviewMesh) -> f32 {
    match mesh {
        PreviewMesh::Plane => PLANE_SIZE,
        PreviewMesh::Sphere | PreviewMesh::Cube | PreviewMesh::Suzanne => 1.0,
    }
}

/// The stand-in `subject` is drawn on, or `None` for a model, which renders as itself.
pub(crate) fn stand_in(mesh: PreviewMesh, subject: &PreviewSubject) -> Option<PreviewMesh> {
    (!matches!(subject, PreviewSubject::Model(_))).then_some(mesh)
}

/// The fixed framing `subject` is shown at on `mesh`: the plane's low view for a
/// subject on the plane, [`OrbitState::default`] for everything else (a model ignores
/// the mesh choice, so it ignores its framing too). `Debug.Preview` renders exactly
/// this; the Inspector's tab starts its orbit here.
pub fn framing(mesh: PreviewMesh, subject: &PreviewSubject) -> OrbitState {
    match stand_in(mesh, subject) {
        Some(PreviewMesh::Plane) => PLANE_ORBIT,
        _ => OrbitState::default(),
    }
}

/// The preview light's rotation for what is shown: a light raking
/// across the plane, travelling +X (across the camera's -Z view), or the classic
/// three-quarter key light.
pub(crate) fn light_pose(shown: Option<PreviewMesh>) -> Quat {
    match shown {
        Some(PreviewMesh::Plane) => {
            let elevation = PLANE_LIGHT_ELEVATION.to_radians();
            let travel = Vec3::new(elevation.cos(), -elevation.sin(), 0.0);
            Quat::from_rotation_arc(Vec3::NEG_Z, travel)
        }
        _ => Quat::from_euler(
            glam::EulerRot::YXZ,
            (-40.0f32).to_radians(),
            (-35.0f32).to_radians(),
            0.0,
        ),
    }
}

/// `material` as it is drawn `repeats` times across `mesh`. Under UV0 its own tiling
/// is multiplied by `repeats`; under a world mapping (`Planar`, `Triplanar`) its
/// `world_size`, and its detail layer's in proportion, are set so that `repeats ×
/// world_size` metres span the stand-in ([`extent`]): the same number of repeats shows
/// either way, and the camera stays where it is. A detail layer under UV0 follows the
/// base's tiled UV already. `repeats` below one is taken as one.
pub fn repeated(mut material: MaterialAsset, mesh: PreviewMesh, repeats: u32) -> MaterialAsset {
    let n = repeats.max(1) as f32;
    match material.uv_mapping {
        UvMapping::Uv0 => material.tiling = material.tiling.map(|t| t * n),
        UvMapping::Planar | UvMapping::Triplanar => {
            let world_size = extent(mesh) / n;
            let factor = world_size / material.world_size;
            material.world_size = world_size;
            if let Some(detail) = material.detail.as_mut() {
                if factor.is_finite() {
                    detail.world_size *= factor;
                }
            }
        }
    }
    material
}

#[cfg(test)]
#[path = "stand_in_tests.rs"]
mod stand_in_tests;
