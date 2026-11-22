//! crates/editor/src/viewport/gizmo.rs — the draggable move gizmo.

use glam::Vec3;

use super::pick::Ray;
use rusty_scene::Scene;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GizmoAxis {
    X,
    Y,
    Z,
}

impl GizmoAxis {
    fn dir(self) -> Vec3 {
        match self {
            GizmoAxis::X => Vec3::X,
            GizmoAxis::Y => Vec3::Y,
            GizmoAxis::Z => Vec3::Z,
        }
    }
}

const AXIS_LENGTH: f32 = 2.0;

#[derive(Clone, Copy, Debug)]
pub struct GizmoDrag {
    pub axis: GizmoAxis,
    last_t: f32,
}

pub fn begin_drag(origin: Vec3, ray: Ray, pick_radius: f32) -> Option<GizmoDrag> {
    let mut best: Option<(GizmoAxis, f32)> = None;
    for axis in [GizmoAxis::X, GizmoAxis::Y, GizmoAxis::Z] {
        let dist = ray_segment_distance(ray, origin, axis.dir(), AXIS_LENGTH);
        if dist <= pick_radius && best.map(|(_, d)| dist < d).unwrap_or(true) {
            best = Some((axis, dist));
        }
    }
    best.map(|(axis, _)| GizmoDrag {
        axis,
        last_t: closest_t_on_axis(ray, origin, axis.dir()),
    })
}

/// Advance a drag for the current pick `ray`: translate `entity_id` along the grabbed
/// axis by the change in the along-axis scalar, writing through the inspector's
/// transform-mutation path (the write queues the collider refresh). Returns whether the
/// entity moved, so the caller can flag the scene dirty.
pub fn drag(
    scene: &mut Scene,
    entity_id: u32,
    drag: &mut GizmoDrag,
    origin: Vec3,
    ray: Ray,
) -> bool {
    let axis = drag.axis.dir();
    let t = closest_t_on_axis(ray, origin, axis);
    let delta = t - drag.last_t;
    drag.last_t = t;
    if delta.abs() < f32::EPSILON {
        return false;
    }
    let Some(mut transform) = scene.world.transform_mut(entity_id) else {
        return false;
    };
    transform.position += axis * delta;
    true
}

/// The scalar `t` such that `origin + t * axis` is the point on the (infinite) axis
/// line closest to `ray`. The drag tracks the delta of this between frames.
fn closest_t_on_axis(ray: Ray, origin: Vec3, axis: Vec3) -> f32 {
    let a = axis;
    let b = ray.dir;
    let w0 = origin - ray.origin;
    let aa = a.dot(a);
    let bb = b.dot(b);
    let ab = a.dot(b);
    let denom = aa * bb - ab * ab;
    if denom.abs() < 1e-6 {
        return 0.0;
    }
    let ac = a.dot(w0);
    let bc = b.dot(w0);
    (ab * bc - bb * ac) / denom
}

/// Distance from `ray` to the axis *segment* `[origin, origin + len*axis]`, for the
/// handle hit-test.
fn ray_segment_distance(ray: Ray, origin: Vec3, axis: Vec3, len: f32) -> f32 {
    let t = closest_t_on_axis(ray, origin, axis).clamp(0.0, len);
    let on_axis = origin + axis * t;
    let s = (on_axis - ray.origin).dot(ray.dir).max(0.0);
    let on_ray = ray.origin + ray.dir * s;
    (on_axis - on_ray).length()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_edit::authoring::{self, Primitive};
    use rusty_scene::Scene;

    fn ray(origin: Vec3, dir: Vec3) -> Ray {
        Ray {
            origin,
            dir: dir.normalize(),
        }
    }

    #[test]
    fn grabs_nearest_axis() {
        let r = ray(Vec3::new(1.0, 0.0, 5.0), Vec3::new(0.0, 0.0, -1.0));
        let grab = begin_drag(Vec3::ZERO, r, 0.3).unwrap();
        assert_eq!(grab.axis, GizmoAxis::X);
    }

    #[test]
    fn miss_returns_none() {
        let r = ray(Vec3::new(50.0, 50.0, 5.0), Vec3::new(0.0, 0.0, -1.0));
        assert!(begin_drag(Vec3::ZERO, r, 0.3).is_none());
    }

    #[test]
    fn drag_translates_along_axis() {
        let mut scene = Scene::new();
        let id = authoring::create_entity(&mut scene, "Box", Some(Primitive::Box));
        let origin = Vec3::ZERO;
        let r0 = ray(Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, -1.0));
        let mut g = GizmoDrag {
            axis: GizmoAxis::X,
            last_t: closest_t_on_axis(r0, origin, Vec3::X),
        };
        let r1 = ray(Vec3::new(2.0, 0.0, 5.0), Vec3::new(0.0, 0.0, -1.0));
        let moved = drag(&mut scene, id, &mut g, origin, r1);
        assert!(moved);
        let pos = scene.world.transform(id).unwrap().position;
        assert!((pos.x - 2.0).abs() < 1e-3, "x moved: {pos:?}");
        assert!(
            pos.y.abs() < 1e-3 && pos.z.abs() < 1e-3,
            "only x moved: {pos:?}"
        );
    }
}
