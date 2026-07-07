//! src/editor/viewport_pick.rs — screen→world ray + click-to-select (#183).
//!
//! Maps a pointer position inside the Scene viewport to a world-space ray using the
//! editor camera's view-projection (the same matrix the renderer draws with), then
//! picks the nearest entity whose mesh AABB the ray hits. Selection sets the editor's
//! `selected_entity_id` exactly as a hierarchy click does, so the inspector and the
//! selection overlay light up identically. Kept egui-free (pure glam) so the math is
//! testable and the `wgpu`/`egui` layers stay decoupled.

use glam::{Mat4, Vec3};

use crate::render::Camera;
use crate::scene::Scene;

/// A world-space ray: an origin on the near plane and a normalized direction.
#[derive(Clone, Copy, Debug)]
pub struct Ray {
    pub origin: Vec3,
    pub dir: Vec3,
}

/// Build a world ray from a normalized device coordinate (`ndc_x`/`ndc_y` in
/// `[-1, 1]`, y up) by un-projecting the near and far plane points through the
/// camera's inverse view-projection. `aspect` must match the viewport the camera was
/// rendered at so the ray lines up with the displayed image.
pub fn ray_from_ndc(camera: &Camera, aspect: f32, ndc_x: f32, ndc_y: f32) -> Ray {
    let view_proj = camera.build_view_projection(aspect);
    let inv = view_proj.inverse();
    let near = inv.project_point3(Vec3::new(ndc_x, ndc_y, 0.0));
    let far = inv.project_point3(Vec3::new(ndc_x, ndc_y, 1.0));
    Ray {
        origin: near,
        dir: (far - near).normalize_or_zero(),
    }
}

/// Convert a pointer offset inside the viewport image (points, top-left origin) to an
/// NDC pair (y flipped to OpenGL/clip up). `size` is the image's size in the same
/// units. Returns `None` for a degenerate (zero-area) image.
pub fn local_to_ndc(local_x: f32, local_y: f32, width: f32, height: f32) -> Option<(f32, f32)> {
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    let ndc_x = (local_x / width) * 2.0 - 1.0;
    let ndc_y = 1.0 - (local_y / height) * 2.0;
    Some((ndc_x, ndc_y))
}

/// Nearest entity hit by `ray`, picking against each active entity's mesh world AABB.
/// Returns the entity id and the ray time-of-impact, or `None` when the ray misses
/// every entity (the caller treats a miss as a deselect). Reused for both edit and
/// play mode — it needs no rapier world, only the scene's geometry.
pub fn pick_entity(scene: &Scene, ray: Ray) -> Option<(u32, f32)> {
    let mut best: Option<(u32, f32)> = None;
    for id in scene.entity_ids() {
        if !scene.world.is_active(id) {
            continue;
        }
        let Some(mesh) = scene.world.mesh(id) else {
            continue;
        };
        let world = scene.compute_world_matrix(id);
        let Some((min, max)) = mesh.world_aabb(world) else {
            continue;
        };
        if let Some(toi) = ray_aabb(ray, min, max) {
            if best.map(|(_, b)| toi < b).unwrap_or(true) {
                best = Some((id, toi));
            }
        }
    }
    best
}

/// Slab-method ray/AABB intersection. Returns the entry time-of-impact when the ray
/// hits the box ahead of its origin (including from inside, `toi = 0`), else `None`.
fn ray_aabb(ray: Ray, min: Vec3, max: Vec3) -> Option<f32> {
    let inv = ray.dir.recip();
    let t0 = (min - ray.origin) * inv;
    let t1 = (max - ray.origin) * inv;
    let tmin = t0.min(t1);
    let tmax = t0.max(t1);
    let enter = tmin.x.max(tmin.y).max(tmin.z);
    let exit = tmax.x.min(tmax.y).min(tmax.z);
    if exit >= enter.max(0.0) {
        Some(enter.max(0.0))
    } else {
        None
    }
}

/// The world-space position of `entity_id` (its world matrix's translation), used to
/// anchor the move gizmo. `None` when the entity is gone.
pub fn entity_world_position(scene: &Scene, entity_id: u32) -> Option<Vec3> {
    if !scene.world.contains(entity_id) {
        return None;
    }
    Some(world_translation(scene.compute_world_matrix(entity_id)))
}

fn world_translation(m: Mat4) -> Vec3 {
    m.col(3).truncate()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::authoring::{self, Primitive};
    use crate::scene::Scene;

    #[test]
    fn ndc_maps_corners() {
        // Top-left of the image is NDC (-1, 1); bottom-right is (1, -1).
        assert_eq!(local_to_ndc(0.0, 0.0, 100.0, 50.0), Some((-1.0, 1.0)));
        assert_eq!(local_to_ndc(100.0, 50.0, 100.0, 50.0), Some((1.0, -1.0)));
        // Centre is the origin.
        assert_eq!(local_to_ndc(50.0, 25.0, 100.0, 50.0), Some((0.0, 0.0)));
    }

    #[test]
    fn ray_through_centre_hits_box_at_origin() {
        let mut scene = Scene::new();
        let id = authoring::create_entity(&mut scene, "Box", Some(Primitive::Box));
        scene.update_all_colliders();
        // A camera looking down -Z from +Z at the origin; the centre ray hits the box.
        let camera = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
        let ray = ray_from_ndc(&camera, 1.0, 0.0, 0.0);
        let hit = pick_entity(&scene, ray);
        assert_eq!(hit.map(|(id, _)| id), Some(id));
    }

    #[test]
    fn empty_space_misses() {
        let mut scene = Scene::new();
        authoring::create_entity(&mut scene, "Box", Some(Primitive::Box));
        let camera = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
        // A corner ray points away from the box at the origin.
        let ray = ray_from_ndc(&camera, 1.0, -0.99, 0.99);
        assert!(pick_entity(&scene, ray).is_none());
    }
}
