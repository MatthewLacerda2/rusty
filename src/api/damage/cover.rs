//! src/api/damage/cover.rs — whether a blast reaches a point (#1893): Unreal's
//! `ComponentIsDamageableFrom`, one cover ray from the origin.
//!
//! A blocker is a solid collider on `blocking_mask` that belongs neither to the
//! group being tested nor to the `causer`. A blocker the origin lies on or in
//! (a grenade at a raycast's `hit.point` on a wall) does not block a ray that
//! leaves it, nor any ray when the origin is buried deeper than [`STEP`] inside
//! it (a ray starting inside a body doesn't hit it); it still blocks a ray from
//! its surface **into** it, so a blast on a wall's face doesn't pass through the
//! wall to the room beyond.

use glam::Vec3;

use super::radial::Blast;
use crate::physics::{is_hittable, PhysicsWorld};
use crate::scene::{layer_in_mask, Scene};
use crate::ui::events::is_under;

/// How short of its target a cover ray may stop and still count as clear (m),
/// and how near the origin must be to a collider to count as on it.
pub(super) const SKIN: f32 = 1e-3;

/// How far along (and back along) the ray the origin's side of a blocker it
/// lies on is probed (m). A wall thinner than this may let a blast on its face
/// through.
const STEP: f32 = 5e-3;

/// Whether `id`'s collider is a solid one on `mask` (`None`: every layer).
pub(super) fn solid_on(scene: &Scene, id: u32, mask: Option<u32>) -> bool {
    let world = &scene.world;
    let trigger =
        world.collider(id).is_some_and(|c| c.is_trigger) && !world.has_character_controller(id);
    is_hittable(scene, id) && !trigger && mask.is_none_or(|m| layer_in_mask(world.layer(id), m))
}

/// Whether `id` lies on the ray's way: always, unless the origin is on or in it
/// and the ray doesn't start into it from its surface.
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

/// Whether a ray from the blast's origin reaches `target` without meeting a
/// blocker (module doc), `owner` being the group tested.
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
