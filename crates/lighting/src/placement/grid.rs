use glam::Vec3;

use super::VERTICAL_HEADROOM;
use rusty_scene::Scene;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProbeGridPlan {
    pub min: Vec3,
    pub max: Vec3,
    pub spacing: f32,
}

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

fn actor_volume((min, mut max): (Vec3, Vec3)) -> (Vec3, Vec3) {
    max.y += VERTICAL_HEADROOM;
    (min, max)
}

fn placement_bounds(scene: &Scene, walkable: Option<(Vec3, Vec3)>) -> Option<(Vec3, Vec3)> {
    let scene_aabb = static_scene_aabb(scene);
    let nav_aabb = walkable.map(actor_volume);
    match (nav_aabb, scene_aabb) {
        (Some((nmin, nmax)), Some((smin, smax))) => {
            let min = nmin.max(smin);
            let max = nmax.min(smax);
            let min = Vec3::new(min.x, nmin.y, min.z);
            let max = Vec3::new(max.x, nmax.y, max.z);
            (max.cmpgt(min).all()).then_some((min, max))
        }
        (Some(b), None) => Some(b),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

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
