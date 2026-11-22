use glam::{Quat, Vec3};

use rusty_components::{MaterialAsset, UvMapping};
use rusty_scene::rules::material::{MAP_COORD_RANGE, WORLD_SIZE_RANGE};

use crate::{OrbitState, PreviewMesh, PreviewSubject};

/// The plane's edge, in metres: square, centred on [`PLANE_CENTRE`].
pub const PLANE_SIZE: f32 = 4.0;
pub const PLANE_CENTRE: Vec3 = Vec3::new(0.0, 0.0, -0.9);
/// The camera's elevation above the plane's surface, in degrees.
pub const PLANE_VIEW_ELEVATION: f32 = 25.0;
/// The raking light's elevation above the plane's surface, in degrees.
pub const PLANE_LIGHT_ELEVATION: f32 = 20.0;

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

pub(crate) fn stand_in(mesh: PreviewMesh, subject: &PreviewSubject) -> Option<PreviewMesh> {
    (!matches!(subject, PreviewSubject::Model(_))).then_some(mesh)
}

pub fn framing(mesh: PreviewMesh, subject: &PreviewSubject) -> OrbitState {
    match stand_in(mesh, subject) {
        Some(PreviewMesh::Plane) => PLANE_ORBIT,
        _ => OrbitState::default(),
    }
}

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
            let base = extent(mesh);
            let ratio = material
                .detail
                .as_ref()
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
