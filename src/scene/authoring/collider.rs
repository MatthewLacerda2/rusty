//! src/scene/authoring/collider.rs — Shared collider-authoring ops.
//!
//! The ONE place the engine knows how to mutate an entity's first-class
//! `ColliderComponent` field by field: the `active` / `is_trigger` flags and the
//! collider `shape` (its variant + extents). The editor's Collider card routes every
//! write through these (#287); there is no Lua `*.Set*` collider field-setter today
//! (`Physics.*` exposes rigidbody + raycast only), so this op set is the single
//! source for the card alone, with unit tests standing in for a convergence test.
//!
//! Each op takes `&mut Entity` and is a no-op when the entity has no collider.
//!
//! Allowed deps: components (the `ColliderComponent`/`ColliderShape` data). Pure.

use crate::components::{ColliderShape, Entity};

/// Set the collider's `active` flag.
pub fn set_active(entity: &mut Entity, active: bool) {
    if let Some(c) = &mut entity.collider {
        c.active = active;
    }
}

/// Set the collider's `is_trigger` flag.
pub fn set_trigger(entity: &mut Entity, is_trigger: bool) {
    if let Some(c) = &mut entity.collider {
        c.is_trigger = is_trigger;
    }
}

/// Replace the collider's shape (variant + extents). The card builds the new shape
/// from its widgets (a selector switch, an edited extent) and hands the whole value
/// here, so geometry editing lives behind one op rather than a field-by-field borrow.
pub fn set_shape(entity: &mut Entity, shape: ColliderShape) {
    if let Some(c) = &mut entity.collider {
        c.shape = shape;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::ColliderComponent;
    use crate::scene::Scene;
    use glam::Vec3;

    fn scene_with_collider() -> (Scene, u32) {
        let mut scene = Scene::new();
        let id = scene.add_entity("Box".to_string());
        if let Some(mut e) = scene.get_entity_mut(id) {
            e.collider = Some(ColliderComponent {
                active: true,
                shape: ColliderShape::Box { size: Vec3::ONE },
                is_trigger: false,
                aabb_min: Vec3::ZERO,
                aabb_max: Vec3::ZERO,
            });
        }
        (scene, id)
    }

    #[test]
    fn ops_write_through() {
        let (mut scene, id) = scene_with_collider();
        let mut e = scene.get_entity_mut(id).unwrap();
        set_active(&mut e, false);
        set_trigger(&mut e, true);
        set_shape(&mut e, ColliderShape::Sphere { radius: 2.0 });
        let c = e.collider.as_ref().unwrap();
        assert!(!c.active);
        assert!(c.is_trigger);
        assert_eq!(c.shape, ColliderShape::Sphere { radius: 2.0 });
    }

    #[test]
    fn op_on_colliderless_entity_is_a_no_op() {
        let mut scene = Scene::new();
        let id = scene.add_entity("Empty".to_string());
        let mut e = scene.get_entity_mut(id).unwrap();
        set_active(&mut e, false);
        assert!(e.collider.is_none());
    }
}
