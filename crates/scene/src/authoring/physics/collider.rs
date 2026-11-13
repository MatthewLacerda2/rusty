//! crates/scene/src/authoring/physics/collider.rs — Shared collider-authoring ops.
//!
//! The ONE place the engine knows how to mutate an entity's first-class
//! `ColliderComponent` field by field: the `active` / `is_trigger` flags, the
//! collider `shape` (its variant + extents), its physics `material` (#447) and the
//! shared asset it names instead (`material_asset`, #1878). The
//! editor's Collider card routes every write through these (#287), and so do the
//! `Physics.SetColliderShape` / `Physics.SetPhysicsMaterial` bindings — one write
//! for both surfaces.
//!
//! Each op takes `&mut ColliderComponent`; the caller's accessor
//! (`world.collider_mut(id)`) carries the no-op-when-absent semantics (#344), and
//! refreshes the world AABB when the edit lands (`ecs::world_space::collider`).
//!
//! Pure, but for `set_material_asset`, which reads the asset it is given.

use crate::authoring::sanitize::{non_negative, unit};
use crate::physics_material;
use rusty_components::{ColliderComponent, ColliderShape, PhysicsMaterial};

/// Set the collider's `active` flag.
pub fn set_active(c: &mut ColliderComponent, active: bool) {
    c.active = active;
}

/// Set the collider's `is_trigger` flag.
pub fn set_trigger(c: &mut ColliderComponent, is_trigger: bool) {
    c.is_trigger = is_trigger;
}

/// Replace the collider's shape (variant + extents). The card builds the new shape
/// from its widgets (a selector switch, an edited extent) and hands the whole value
/// here, so geometry editing lives behind one op rather than a field-by-field borrow.
pub fn set_shape(c: &mut ColliderComponent, shape: ColliderShape) {
    c.shape = shape;
}

/// Replace the collider's inline physics material, clamping its coefficients into
/// range (friction `>= 0`, bounciness `[0, 1]`) so neither surface can author an
/// out-of-range value. It never touches `material_asset`: a scene load re-runs this
/// op on a clamped inline value (`fields::hold`), and that must keep the asset. A
/// caller that means "give the collider its own values" (`Physics.SetPhysicsMaterial`,
/// Unity's `Collider.material`) clears the asset itself.
// lint: allow(sanitize) both coefficients go through a helper below
pub fn set_material(c: &mut ColliderComponent, material: PhysicsMaterial) {
    c.material = PhysicsMaterial {
        friction_combine: material.friction_combine,
        bounce_combine: material.bounce_combine,
        ..PhysicsMaterial::new(non_negative(material.friction), unit(material.bounciness))
    };
}

/// Name the shared physics material asset the collider uses (#1878), or `None`
/// (or an empty path) for its inline values. The path is stored in its one form
/// (`physics_material::key`), since it is the key every hit reports, and is
/// refused when no asset loads from it: the one check `Physics.SetMaterial`, the
/// inspector and `Component.Set` all go through. A scene file is not checked (it
/// is read, not written through ops), so a dangling path loaded from one is kept
/// and falls back to the default material.
pub fn set_material_asset(c: &mut ColliderComponent, path: Option<String>) -> Result<(), String> {
    let path = path
        .map(|p| physics_material::key(&p))
        .filter(|p| !p.is_empty());
    if let Some(p) = &path {
        if physics_material::lookup(p).is_none() {
            return Err(format!("no physics material asset loads from {p:?}"));
        }
    }
    c.material_asset = path;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::Scene;

    fn scene_with_collider() -> (Scene, u32) {
        let mut scene = Scene::new();
        let id = scene.add_entity("Box".to_string());
        let c = crate::authoring::default_collider();
        scene.world.set_collider(id, Some(c));
        (scene, id)
    }

    #[test]
    fn ops_write_through() {
        let (mut scene, id) = scene_with_collider();
        let mut e = scene.world.collider_mut(id).unwrap();
        set_active(&mut e, false);
        set_trigger(&mut e, true);
        set_shape(&mut e, ColliderShape::Sphere { radius: 2.0 });
        let c = &*e;
        assert!(!c.active);
        assert!(c.is_trigger);
        assert_eq!(c.shape, ColliderShape::Sphere { radius: 2.0 });
    }

    #[test]
    fn set_material_writes_modes_and_clamps_coefficients() {
        use rusty_components::CombineMode;
        let (mut scene, id) = scene_with_collider();
        let mut e = scene.world.collider_mut(id).unwrap();
        let wanted = PhysicsMaterial {
            friction: -1.0,
            bounciness: 2.0,
            friction_combine: CombineMode::Minimum,
            bounce_combine: CombineMode::Maximum,
        };
        set_material(&mut e, wanted);
        assert_eq!(
            e.material,
            PhysicsMaterial {
                friction: 0.0,
                bounciness: 1.0,
                ..wanted
            }
        );
        set_material(&mut e, PhysicsMaterial::new(0.7, 0.25));
        assert_eq!((e.material.friction, e.material.bounciness), (0.7, 0.25));
    }

    #[test]
    fn op_on_colliderless_entity_is_a_no_op() {
        let mut scene = Scene::new();
        let id = scene.add_entity("Empty".to_string());
        // The accessor is the no-op gate now: no collider, no guard, no write.
        if let Some(mut c) = scene.world.collider_mut(id) {
            set_active(&mut c, false);
        }
        assert!(!scene.world.has_collider(id));
    }
}
