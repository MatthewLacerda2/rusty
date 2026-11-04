//! src/render/lod.rs — which LODGroup levels a view shows (#472).
//!
//! Per view, every active [`LodGroupComponent`] measures how much of the screen its
//! `size` covers and picks one level (`LodGroupComponent::level_at`); the renderers
//! of every *other* level are hidden from that view. The result is a set of hidden
//! entity ids the solid pass and the shadow sweeps skip **before** they batch, so
//! the instances of one level of one prop still share an instanced draw (#470).
//!
//! Render-only: this reads the scene and writes nothing back, so the sim never sees
//! which level is shown and determinism is untouched.
//!
//! Shadows: the dynamic casters use the **base camera's** selection, so a shadow
//! never pops to a different level than the object casting it. The static bake is
//! cached across frames (#355) and re-baked only when a cascade moves, so it cannot
//! follow the camera: it bakes every group at LOD0 ([`LodSelection::finest`]).

use std::collections::HashSet;

use glam::Mat4;

use crate::components::{LodGroupComponent, Projection};
use crate::scene::{Camera, Scene};

/// The entities one view hides because their LOD level is not the one shown.
#[derive(Debug, Default)]
pub(crate) struct LodSelection {
    hidden: HashSet<u32>,
}

impl LodSelection {
    /// The selection `cam` sees: each group's level picked by its screen height.
    pub(crate) fn for_camera(scene: &Scene, cam: &Camera) -> Self {
        Self::build(scene, |group, world| {
            let height = screen_height(group.size * max_scale(world), cam, world);
            group.level_at(height)
        })
    }

    /// Every group at its finest level — what the cached static shadow bake draws.
    pub(crate) fn finest(scene: &Scene) -> Self {
        Self::build(scene, |_, _| Some(0))
    }

    /// Whether `id` is a renderer of a level this view does not show.
    pub(crate) fn hides(&self, id: u32) -> bool {
        self.hidden.contains(&id)
    }

    /// Hide, per active group, every renderer not in the level `pick` chooses
    /// (`None`: culled, so all of them). An entity listed in the shown level stays
    /// visible even when another level lists it too.
    fn build(scene: &Scene, pick: impl Fn(&LodGroupComponent, Mat4) -> Option<usize>) -> Self {
        let mut hidden = HashSet::new();
        for id in scene.world.ids_with_lod_group() {
            if !scene.world.is_active_in_hierarchy(id) {
                continue;
            }
            let Some(group) = scene.world.lod_group(id) else {
                continue;
            };
            let shown =
                pick(&group, scene.world.local_to_world(id)).and_then(|i| group.levels.get(i));
            let keep = |r: &u32| shown.is_some_and(|l| l.renderers.contains(r));
            hidden.extend(group.renderers().filter(|r| !keep(r)));
        }
        Self { hidden }
    }
}

/// The fraction of `cam`'s viewport height a `size`-metre object at `world`'s origin
/// covers ([`relative_height`] at its distance from the camera).
pub(crate) fn screen_height(size: f32, cam: &Camera, world: Mat4) -> f32 {
    let distance = world.w_axis.truncate().distance(cam.position);
    relative_height(size, distance, cam.fov, cam.projection)
}

/// The fraction of the viewport height a `size`-metre object `distance` metres from
/// a camera of vertical `fov` (degrees) covers: `size / (2 · distance · tan(fov / 2))`,
/// Unity's `LODGroup` relative height (#472). At the camera it is infinite (always the
/// finest level, never culled). Orthographic: `size / (2 · half-height)`, whatever the
/// distance. Small-object culling (#1714) measures with it too, so a *Min Screen
/// Size* means what a `LODGroup` threshold means.
pub(crate) fn relative_height(size: f32, distance: f32, fov: f32, projection: Projection) -> f32 {
    // An orthographic view (#430) shows everything at one scale, whatever the distance.
    if let Projection::Orthographic { size: half } = projection {
        return size / (2.0 * half);
    }
    let half_fov = (fov.to_radians() * 0.5).tan();
    match distance * half_fov {
        d if d > 0.0 => size / (2.0 * d),
        _ => f32::INFINITY,
    }
}

/// The largest axis scale in `world` — how much a group's authored size, or a mesh's
/// local extent (#1714), grows.
pub(crate) fn max_scale(world: Mat4) -> f32 {
    let axes = [world.x_axis, world.y_axis, world.z_axis];
    axes.iter()
        .map(|a| a.truncate().length())
        .fold(0.0, f32::max)
}

#[cfg(test)]
#[path = "lod_tests.rs"]
mod lod_tests;

#[cfg(test)]
#[path = "lod_gpu_tests.rs"]
mod lod_gpu_tests;
