use rusty_components::{ColliderComponent, ColliderShape, PhysicsMaterial};
use rusty_scene::physics_material;
use rusty_scene::rules::sanitize::{non_negative, unit};

pub fn set_active(c: &mut ColliderComponent, active: bool) {
    c.active = active;
}

pub fn set_trigger(c: &mut ColliderComponent, is_trigger: bool) {
    c.is_trigger = is_trigger;
}

/// Through `World::collider_mut`, whose guard refreshes the world AABB when the shape changes.
pub fn set_shape(c: &mut ColliderComponent, shape: ColliderShape) {
    c.shape = shape;
}

/// Never touches `material_asset`: a scene load re-runs this op (`fields::hold`), which must keep
/// the asset.
// lint: allow(sanitize) both coefficients go through a helper below
pub fn set_material(c: &mut ColliderComponent, material: PhysicsMaterial) {
    c.material = PhysicsMaterial {
        friction_combine: material.friction_combine,
        bounce_combine: material.bounce_combine,
        ..PhysicsMaterial::new(non_negative(material.friction), unit(material.bounciness))
    };
}

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

    use rusty_scene::Scene;

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
        if let Some(mut c) = scene.world.collider_mut(id) {
            set_active(&mut c, false);
        }
        assert!(!scene.world.has_collider(id));
    }
}
