//! Shared helpers for the navigation tests: real box colliders (transform + shape,
//! so the bake rasterises what physics would collide with) and span lookups.

use super::{NavBounds, NavigationGraph, SpanRef};
use crate::scene::{ColliderComponent, ColliderShape, Scene};
use glam::{Quat, Vec3};

/// A box collider filling the world AABB `min..max`; `is_static` picks bake eligibility.
pub fn add_box_with(scene: &mut Scene, min: Vec3, max: Vec3, is_static: bool) -> u32 {
    add_rotated_box(
        scene,
        (min + max) * 0.5,
        max - min,
        Quat::IDENTITY,
        is_static,
    )
}

/// A static box collider filling the world AABB `min..max`.
pub fn add_box(scene: &mut Scene, min: Vec3, max: Vec3) -> u32 {
    add_box_with(scene, min, max, true)
}

/// A box of `size` centred at `center`, turned by `rot` (a ramp, when tilted).
pub fn add_rotated_box(
    scene: &mut Scene,
    center: Vec3,
    size: Vec3,
    rot: Quat,
    is_static: bool,
) -> u32 {
    let id = scene.add_entity("box".to_string());
    scene.world.set_static(id, is_static);
    if let Some(mut t) = scene.world.transform_mut(id) {
        t.position = center;
        t.rotation = rot;
    }
    let mut col = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
    col.shape = ColliderShape::Box { size };
    scene.world.set_collider(id, Some(col));
    scene.update_entity_collider(id);
    id
}

/// A 0.1-thick floor slab whose top is `y = 0` over `x0..x1`, `z0..z1`.
pub fn add_floor(scene: &mut Scene, x0: f32, x1: f32, z0: f32, z1: f32) -> u32 {
    add_box(scene, Vec3::new(x0, -0.1, z0), Vec3::new(x1, 0.0, z1))
}

/// Bake `scene` onto the pinned 0..10 × 0..10 unit grid most tests assert cells on.
pub fn bake_pinned(scene: &mut Scene) -> NavigationGraph {
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 10.0, 0.0, 10.0));
    let mut g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
    g.bake(scene);
    g
}

/// The lowest span of a cell (panics when the cell has none).
pub fn ground(g: &NavigationGraph, gx: i32, gz: i32) -> SpanRef {
    let index = g.span_range(gx, gz).start;
    assert!(g.is_walkable(gx, gz), "cell ({gx},{gz}) has no span");
    SpanRef {
        gx,
        gz,
        index: index as u32,
    }
}

/// The floor heights of a cell's spans, bottom-up.
pub fn floors(g: &NavigationGraph, gx: i32, gz: i32) -> Vec<f32> {
    g.spans_at(gx, gz).iter().map(|s| s.y).collect()
}
