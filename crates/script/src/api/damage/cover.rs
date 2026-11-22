use glam::Vec3;

use super::radial::Blast;
use rusty_physics::{is_hittable, PhysicsWorld};
use rusty_scene::{layer_in_mask, Scene};
use rusty_ui::events::is_under;

pub(super) const SKIN: f32 = 1e-3;

const STEP: f32 = 5e-3;

pub(super) fn solid_on(scene: &Scene, id: u32, mask: Option<u32>) -> bool {
    let world = &scene.world;
    let trigger =
        world.collider(id).is_some_and(|c| c.is_trigger) && !world.has_character_controller(id);
    is_hittable(scene, id) && !trigger && mask.is_none_or(|m| layer_in_mask(world.layer(id), m))
}

fn in_the_way(physics: &PhysicsWorld, id: u32, origin: Vec3, dir: Vec3) -> bool {
    let on = physics
        .closest_point_on(id, origin)
        .is_some_and(|p| p.distance(origin) <= SKIN);
    if !on {
        return true;
    }
    let inside = |p: Vec3| physics.collider_contains_point(id, p) == Some(true);
    inside(origin + dir * STEP) && !inside(origin - dir * STEP)
}

pub(super) fn clear(
    scene: &Scene,
    physics: &PhysicsWorld,
    (blast, causer): (&Blast, Option<u32>),
    owner: u32,
    target: Vec3,
) -> bool {
    let to = target - blast.origin;
    let length = to.length();
    if length <= SKIN {
        return true;
    }
    let dir = to / length;
    let world = &scene.world;
    let blocks = |id: u32| {
        solid_on(scene, id, blast.blocking_mask)
            && !is_under(world, id, owner)
            && causer.is_none_or(|c| !is_under(world, id, c))
            && in_the_way(physics, id, blast.origin, dir)
    };
    physics
        .raycast_hit(blast.origin, to, length, blocks)
        .is_none_or(|hit| hit.distance >= length - SKIN)
}
