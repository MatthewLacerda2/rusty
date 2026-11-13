//! Line of sight and the camera's view (#1892): the `trace` and `view` tests.

use glam::{Vec2, Vec3};

use super::QueryWorld;
use crate::ray;
use rusty_components::Projection;
use rusty_scene::Camera;

/// Whether the segment `eye → at` is clear of colliders on `mask` ([`ray::clear`]).
pub fn clear(
    world: &QueryWorld,
    eye: Vec3,
    at: Vec3,
    ignore: [Option<u32>; 2],
    mask: Option<u32>,
) -> bool {
    ray::clear(world.scene, world.physics, (eye, at), ignore, mask)
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
