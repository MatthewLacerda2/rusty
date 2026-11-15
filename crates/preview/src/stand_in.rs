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
use rusty_scene::rules::material::{MAP_COORD_RANGE, WORLD_SIZE_RANGE};

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

/// `material` as it is drawn `repeats` times across `mesh`, or as it is authored when
/// `repeats` is `None` (a world-mapped material then shows its real scale: its true
/// texel density on the stand-in). Under UV0 its own tiling is multiplied by
/// `repeats`; under a world mapping (`Planar`, `Triplanar`) its `world_size`, and its
/// detail layer's in proportion, are set so that `repeats × world_size` metres span
/// the stand-in ([`extent`]): the same number of repeats shows either way, and the
/// camera stays where it is. A detail layer under UV0 follows the base's tiled UV
/// already.
///
/// Errors on `repeats = 0`, and on a count whose tiling or world size falls outside
/// what the renderer draws ([`MAP_COORD_RANGE`], [`WORLD_SIZE_RANGE`]): it would clamp
/// the value and show a different count than was asked for, quietly. The error names
/// the limit and the count that fits.
pub fn repeated(
    mut material: MaterialAsset,
    mesh: PreviewMesh,
    repeats: Option<u32>,
) -> Result<MaterialAsset, String> {
    let Some(repeats) = repeats else {
        return Ok(material);
    };
    if repeats == 0 {
        return Err("preview repeats must be 1 or more, got 0".to_string());
    }
    let n = repeats as f32;
    let on = mesh.name().to_lowercase();
    match material.uv_mapping {
        UvMapping::Uv0 => {
            let tiling = material.tiling.map(|t| t * n);
            if tiling.iter().any(|t| !MAP_COORD_RANGE.contains(t)) {
                let widest = material.tiling.iter().fold(0.0f32, |m, t| m.max(t.abs()));
                return Err(format!(
                    "preview repeats = {repeats} would tile the material {tiling:?} times, \
                     outside the renderer's {}..={}; at most {} repeats fit its tiling {:?}",
                    MAP_COORD_RANGE.start(),
                    MAP_COORD_RANGE.end(),
                    (MAP_COORD_RANGE.end() / widest).floor(),
                    material.tiling,
                ));
            }
            material.tiling = tiling;
        }
        UvMapping::Planar | UvMapping::Triplanar => {
            // Each rescaled size is `k / n` metres: the base's `k` is the stand-in's
            // extent, the detail's that times its ratio to the base.
            let base = extent(mesh);
            let ratio = material
                .detail
                .as_ref()
                // A detail size the renderer already clamps on the material itself is
                // the material's own, not the preview's: left as it is.
                .filter(|d| WORLD_SIZE_RANGE.contains(&d.world_size))
                .map(|d| d.world_size / material.world_size)
                .filter(|r| r.is_finite());
            let sizes = [
                ("world_size", Some(base)),
                ("detail world_size", ratio.map(|r| base * r)),
            ];
            for (what, k) in sizes {
                let Some(k) = k else { continue };
                check_world_size(what, k, repeats, &on)?;
            }
            material.world_size = base / n;
            if let (Some(detail), Some(ratio)) = (material.detail.as_mut(), ratio) {
                detail.world_size = base * ratio / n;
            }
        }
    }
    Ok(material)
}

/// Refuse `repeats` when `k / repeats` metres falls outside [`WORLD_SIZE_RANGE`],
/// naming the count that fits.
fn check_world_size(what: &str, k: f32, repeats: u32, on: &str) -> Result<(), String> {
    let size = k / repeats as f32;
    if WORLD_SIZE_RANGE.contains(&size) {
        return Ok(());
    }
    let (min, max) = (WORLD_SIZE_RANGE.start(), WORLD_SIZE_RANGE.end());
    let fits = if size < *min {
        format!("at most {} repeats fit", (k / min).floor())
    } else {
        format!("at least {} repeats are needed", (k / max).ceil())
    };
    Err(format!(
        "preview repeats = {repeats} on the {on} would draw its {what} at {size} m, \
         outside the renderer's {min}..={max} m; {fits}"
    ))
}

#[cfg(test)]
#[path = "stand_in_tests.rs"]
mod stand_in_tests;

#[cfg(test)]
#[path = "repeats_tests.rs"]
mod repeats_tests;
