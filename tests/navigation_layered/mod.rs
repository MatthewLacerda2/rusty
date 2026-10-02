//! Layered navigation (#454), end to end through the public `NavigationGraph`
//! surface: stacked floors, a bridge over a walkway, real ramps, agents climbing
//! between floors, and byte-identical replay. Levels are blocked out from primitive
//! boxes only, as every engine test level is.

mod bridge;
mod building;
mod level;
mod links;
mod measure;
mod rebake;

use glam::{Quat, Vec3};
use rusty::navigation::{NavBounds, NavigationGraph, SpanRef};
use rusty::scene::{ColliderComponent, ColliderShape, Scene};

/// A static box of `size` centred at `center`, turned by `rot`.
pub fn add_rotated_box(scene: &mut Scene, center: Vec3, size: Vec3, rot: Quat) -> u32 {
    let id = scene.add_entity("box".to_string());
    scene.world.set_static(id, true);
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

/// A static box filling the world AABB `min..max`.
pub fn add_box(scene: &mut Scene, min: Vec3, max: Vec3) -> u32 {
    add_rotated_box(scene, (min + max) * 0.5, max - min, Quat::IDENTITY)
}

/// A ramp along +x whose top surface rises from `(x0, y0)` to `(x1, y1)` over
/// `z0..z1`: a 0.2-thick box tilted about z.
pub fn add_ramp(scene: &mut Scene, (x0, y0): (f32, f32), (x1, y1): (f32, f32), z: (f32, f32)) {
    let (run, rise) = (x1 - x0, y1 - y0);
    let angle = rise.atan2(run);
    let rot = Quat::from_rotation_z(angle);
    let top_mid = Vec3::new((x0 + x1) * 0.5, (y0 + y1) * 0.5, (z.0 + z.1) * 0.5);
    let center = top_mid - rot * Vec3::new(0.0, 0.1, 0.0);
    let size = Vec3::new((run * run + rise * rise).sqrt(), 0.2, z.1 - z.0);
    add_rotated_box(scene, center, size, rot);
}

/// Bake `scene` over `bounds` at its own settings.
pub fn bake(scene: &mut Scene, bounds: NavBounds) -> NavigationGraph {
    scene.nav_settings.bounds = Some(bounds);
    NavigationGraph::from_scene(scene)
}

/// The floor heights along a span path.
pub fn heights(g: &NavigationGraph, path: &[SpanRef]) -> Vec<f32> {
    path.iter().map(|s| g.spans[s.index as usize].y).collect()
}
