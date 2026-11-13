//! The regular light-probe grid (#246): the volume actors move through, filled at
//! one spacing. What the bake places when the level has no static solid to build an
//! occupancy grid from (`light.rs` places the rest, #1737), and the layout
//! `Probe.FillGrid` makes by hand.

use glam::Vec3;

use super::VERTICAL_HEADROOM;
use crate::Scene;

/// A light-probe grid request: the inclusive `[min, max]` corners and the cell spacing,
/// ready to hand to [`crate::lighting::probe::ProbeVolume::fill_grid`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProbeGridPlan {
    pub min: Vec3,
    pub max: Vec3,
    pub spacing: f32,
}

/// World-space AABB of the scene's STATIC, active, solid geometry (its colliders;
/// a trigger is not geometry). `None`
/// when the scene has no static collider to bound — the caller then has nothing to
/// place against. Deterministic: a pure fold over the entities in id order.
pub fn static_scene_aabb(scene: &Scene) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    let mut found = false;
    for id in scene.world.ids_with_collider() {
        if !scene.world.is_static(id) || !scene.world.is_active_in_hierarchy(id) {
            continue;
        }
        let collider = scene
            .world
            .collider(id)
            .expect("id came from ids_with_collider");
        if !collider.active || collider.is_trigger {
            continue;
        }
        let Some(pose) = scene.world.world_pose(id) else {
            continue;
        };
        let (cmin, cmax) = collider.calculate_world_aabb(pose);
        min = min.min(cmin);
        max = max.max(cmax);
        found = true;
    }
    found.then_some((min, max))
}

/// The volume actors move through above a walkable extent: its floors, raised by
/// `VERTICAL_HEADROOM` so probes cover heads and torsos, not only feet.
fn actor_volume((min, mut max): (Vec3, Vec3)) -> (Vec3, Vec3) {
    max.y += VERTICAL_HEADROOM;
    (min, max)
}

/// Choose the light-probe grid bounds: the navigable volume intersected with the
/// scene's static AABB when walkable bounds are given, else the static AABB alone,
/// else `None` (nothing to place). The intersection keeps the grid inside the level
/// rather than spilling past its walls when the nav extent is larger.
fn placement_bounds(scene: &Scene, walkable: Option<(Vec3, Vec3)>) -> Option<(Vec3, Vec3)> {
    let scene_aabb = static_scene_aabb(scene);
    let nav_aabb = walkable.map(actor_volume);
    match (nav_aabb, scene_aabb) {
        (Some((nmin, nmax)), Some((smin, smax))) => {
            let min = nmin.max(smin);
            let max = nmax.min(smax);
            // Keep the nav y-range (actor volume) even if it exceeds the geometry box.
            let min = Vec3::new(min.x, nmin.y, min.z);
            let max = Vec3::new(max.x, nmax.y, max.z);
            (max.cmpgt(min).all()).then_some((min, max))
        }
        (Some(b), None) => Some(b),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Plan the light-probe grid for `scene`. `walkable` is the `[min, max]` box of the
/// spans agents stand on (`NavigationGraph::walkable_aabb`), `None` without a baked
/// nav surface. `spacing` is the grid cell size in world units (clamped to a sane
/// minimum). Returns `None` when there is nothing to bound (an empty/static-less
/// scene), in which case the caller places no light probes.
pub fn plan_light_probes(
    scene: &Scene,
    walkable: Option<(Vec3, Vec3)>,
    spacing: f32,
) -> Option<ProbeGridPlan> {
    let (min, max) = placement_bounds(scene, walkable)?;
    Some(ProbeGridPlan {
        min,
        max,
        spacing: spacing.max(0.5),
    })
}
