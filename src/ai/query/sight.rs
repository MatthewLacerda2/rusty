//! Line of sight and the camera's view (#1892): the `trace` and `view` tests.

use glam::{Vec2, Vec3};

use super::QueryWorld;
use crate::components::Projection;
use crate::physics::is_hittable;
use crate::scene::{layer_in_mask, Camera};

/// Whether the segment `eye → at` is clear: no collider on `mask`'s layers in the
/// way. The entities in `ignore` and their descendants (a character's hitboxes)
/// never block, nor do triggers. With no physics world nothing blocks.
pub fn clear(
    world: &QueryWorld,
    eye: Vec3,
    at: Vec3,
    ignore: [Option<u32>; 2],
    mask: Option<u32>,
) -> bool {
    let Some(physics) = world.physics else {
        return true;
    };
    let way = at - eye;
    let length = way.length();
    if length < 1e-4 {
        return true;
    }
    let scene = world.scene;
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

/// Whether `p` is inside `camera`'s view on a `screen`-pixel screen, its frustum
/// widened by `margin` degrees on every side (a perspective camera; an
/// orthographic one takes its screen rectangle as it is). A point behind the near
/// plane or past the far one is outside.
pub fn in_view(camera: &Camera, screen: Vec2, p: Vec3, margin: f32) -> bool {
    if let Projection::Orthographic { .. } = camera.projection {
        return camera
            .world_to_screen(p, screen)
            .is_on_screen(screen.max(Vec2::ONE));
    }
    let local = camera.view_matrix().transform_point3(p);
    let depth = -local.z;
    if depth < camera.near || depth > camera.far {
        return false;
    }
    let screen = screen.max(Vec2::ONE);
    let half_v = (0.5 * camera.fov).to_radians();
    let half_h = (half_v.tan() * screen.x / screen.y).atan();
    let widen = |half: f32| (half + margin.to_radians()).clamp(0.0, 1.57).tan();
    local.x.abs() <= depth * widen(half_h) && local.y.abs() <= depth * widen(half_v)
}
