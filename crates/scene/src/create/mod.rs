//! crates/scene/src/create/mod.rs — a new GameObject: the Create menu's primitives,
//! the "Add Component" defaults, and the one verb that makes an entity from them.
//!
//! These sit below `edit::authoring` because the base builds with them too (a
//! ragdoll's bodies, a camera's default lens, the state inventory's marks), and
//! `authoring` re-exports every item here, so the editor and the API reach them
//! through its one entry point.

pub mod defaults;
mod primitive;

pub use defaults::{
    attach_default_material, default_animator, default_camera, default_collider, default_light,
    default_material, default_nav_agent, default_post_processing, default_rigidbody,
};
pub use primitive::{
    primitive_collider, primitive_geometry, primitive_mesh_component, primitive_revision,
    subdivided_geometry, Geometry, Primitive,
};

use crate::Scene;

/// Create an entity named `name`, configured for `primitive` as Unity's
/// `GameObject.CreatePrimitive` does (#2051): a mesh primitive gets a `MeshComponent`
/// and the collider matching it ([`primitive_collider`]), a light primitive a
/// `LightComponent`, `None` a bare transform-only entity. Returns the new entity's
/// stable id. The hierarchy's Create menu and the `Scene.CreateEntity` API both
/// call this, so editor and API behaviour match exactly; a part meant to be drawn
/// only removes its collider afterwards, as in Unity.
pub fn create_entity(scene: &mut Scene, name: &str, primitive: Option<Primitive>) -> u32 {
    let id = scene.add_entity(name.to_string());
    let Some(primitive) = primitive else {
        return id;
    };
    if let Some(mesh) = primitive_mesh_component(primitive) {
        let collider = primitive_collider(primitive, &mesh);
        scene.world.set_mesh(id, Some(mesh));
        scene.world.set_collider(id, collider);
    } else if let Some(light) = primitive::primitive_light(primitive) {
        scene.world.set_light(id, Some(light));
    }
    id
}

/// [`create_entity`] without the collider a mesh primitive comes with: a part the
/// engine builds to be drawn only (a preview subject, a water surface, a body a
/// `CharacterController` carries).
pub fn create_drawn_entity(scene: &mut Scene, name: &str, primitive: Option<Primitive>) -> u32 {
    let id = create_entity(scene, name, primitive);
    scene.world.set_collider(id, None);
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LightType;
    use glam::Vec3;

    #[test]
    fn create_entity_configures_per_primitive() {
        let mut scene = Scene::new();
        let bare = create_entity(&mut scene, "Empty", None);
        let boxed = create_entity(&mut scene, "Crate", Some(Primitive::Box));
        let lamp = create_entity(&mut scene, "Lamp", Some(Primitive::PointLight));

        assert!(
            !scene.world.has_mesh(bare) && !scene.world.has_light(bare),
            "bare = transform only"
        );
        let mesh = scene.world.mesh(boxed).expect("box gets a mesh");
        assert_eq!(mesh.primitive_type, "Box");
        assert!(!mesh.vertices.is_empty() && !scene.world.has_light(boxed));
        assert!(!scene.world.has_mesh(lamp));
        assert_eq!(
            scene.world.light(lamp).unwrap().light_type,
            LightType::Point
        );
    }

    /// Each mesh primitive comes with the collider Unity's `CreatePrimitive` gives
    /// it, sized to the engine's mesh (#2051); a light gets none.
    #[test]
    fn create_entity_gives_each_mesh_primitive_its_collider() {
        use rusty_components::{CapsuleAxis, ColliderShape};
        let capsule = ColliderShape::Capsule {
            radius: 0.5,
            height: 2.0,
            axis: CapsuleAxis::Y,
        };
        let plane = ColliderShape::Mesh {
            convex: false,
            local_min: Vec3::new(-7.5, 0.0, -7.5),
            local_max: Vec3::new(7.5, 0.0, 7.5),
        };
        let mut scene = Scene::new();
        for (primitive, shape) in [
            (Primitive::Box, ColliderShape::Box { size: Vec3::ONE }),
            (Primitive::Sphere, ColliderShape::Sphere { radius: 0.5 }),
            (Primitive::Capsule, capsule.clone()),
            (Primitive::Cylinder, capsule),
            (Primitive::Plane, plane),
        ] {
            let id = create_entity(&mut scene, "Part", Some(primitive));
            let c = scene.world.collider(id).expect("a mesh primitive is solid");
            assert_eq!(c.shape, shape, "{primitive:?}");
            assert!(c.active && !c.is_trigger, "{primitive:?}");
        }
        for light in [
            Primitive::PointLight,
            Primitive::DirectionalLight,
            Primitive::SpotLight,
        ] {
            let id = create_entity(&mut scene, "Lamp", Some(light));
            assert!(!scene.world.has_collider(id), "{light:?} gets no collider");
        }
        let bare = create_entity(&mut scene, "Empty", None);
        assert!(!scene.world.has_collider(bare));
    }

    /// The mesh a sized collider stands for spans exactly that size: a mesh builder
    /// retuned without its collider fails here.
    #[test]
    fn primitive_colliders_match_their_meshes() {
        use crate::rehydrate::bounds_of;
        for primitive in [
            Primitive::Box,
            Primitive::Sphere,
            Primitive::Capsule,
            Primitive::Cylinder,
        ] {
            let mesh = primitive_mesh_component(primitive).unwrap();
            let (lo, hi) = bounds_of(mesh.vertices.iter().map(|v| v.position)).unwrap();
            let c = primitive_collider(primitive, &mesh).unwrap();
            let (n, x) = c.shape.scaled(Vec3::ONE).local_box();
            assert!(
                lo.abs_diff_eq(n, 1e-4) && hi.abs_diff_eq(x, 1e-4),
                "{primitive:?}: {lo}..{hi} vs {n}..{x}"
            );
        }
    }

    #[test]
    fn create_drawn_entity_keeps_the_mesh_and_drops_the_collider() {
        let mut scene = Scene::new();
        let id = create_drawn_entity(&mut scene, "Water", Some(Primitive::Plane));
        assert!(scene.world.has_mesh(id) && !scene.world.has_collider(id));
    }

    #[test]
    fn primitive_parse_is_case_insensitive() {
        assert_eq!(Primitive::parse("BOX"), Some(Primitive::Box));
        assert_eq!(Primitive::parse("spotlight"), Some(Primitive::SpotLight));
        assert_eq!(Primitive::parse("Capsule"), Some(Primitive::Capsule));
        assert_eq!(Primitive::parse("nope"), None);
    }
}
