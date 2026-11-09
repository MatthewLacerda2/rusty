//! src/ai/ray.rs — the line-of-sight ray every AI sense shares (#1892, #1900):
//! the environment query's `trace` test and perception's sight sense.

use glam::Vec3;

use crate::physics::{is_hittable, PhysicsWorld};
use crate::scene::{layer_in_mask, Scene};

/// Whether the segment `eye → at` is clear: no collider on `mask`'s layers in the
/// way. The entities in `ignore` and their descendants (a character's hitboxes)
/// never block, nor do triggers. With no physics world nothing blocks.
pub fn clear(
    scene: &Scene,
    physics: Option<&PhysicsWorld>,
    (eye, at): (Vec3, Vec3),
    ignore: [Option<u32>; 2],
    mask: Option<u32>,
) -> bool {
    let Some(physics) = physics else {
        return true;
    };
    let way = at - eye;
    let length = way.length();
    if length < 1e-4 {
        return true;
    }
    let w = &scene.world;
    // An ignored entity's own colliders and its descendants' (a character's
    // hitboxes), never its parent's: a level grouped under an empty parent
    // keeps its walls when the Player sits in the same group.
    let ignored = |id: u32| w.self_and_ancestors(id).any(|a| ignore.contains(&Some(a)));
    let accept = |id: u32| {
        if !is_hittable(scene, id) || ignored(id) {
            return false;
        }
        if w.collider(id).is_some_and(|c| c.is_trigger) && !w.has_character_controller(id) {
            return false;
        }
        mask.is_none_or(|m| layer_in_mask(w.layer(id), m))
    };
    physics.raycast_hit(eye, way, length, accept).is_none()
}
