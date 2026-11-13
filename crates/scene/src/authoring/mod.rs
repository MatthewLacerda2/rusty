//! crates/scene/src/authoring/mod.rs — Shared structural-authoring verbs.
//!
//! The one place the engine knows how to change scene *structure*: create an
//! entity (optionally as one of the hierarchy toolbar's primitives), attach or
//! detach a first-class component with the inspector's "Add Component" defaults,
//! and the primitives the Create menu offers (`primitive`: their mesh, light and
//! collider, and the geometry shared with serialization's rehydrate path).
//!
//! BOTH callers route through here so editor and API can never drift: the editor
//! (`hierarchy.rs` create, `inspector/components/add/` add-menu) and the API surface
//! (`api::scene`), which the console REPL and headless session drive against the
//! live edit world. Parenting/destroy/save have their own canonical `Scene` entry
//! points (`set_parent` / `destroy_entity` / `save_to_file`); the structural API
//! verbs call straight into those, so they are not duplicated here.

pub mod ai;
pub mod animator;
pub mod audio;
pub mod components;
pub mod defaults;
pub mod dependency;
pub mod environment;
pub mod fields;
pub mod fog;
pub mod material;
pub mod nav;
pub mod particles;
pub mod physics;
mod primitive;
pub mod rendering;
pub mod ribbon;
mod sanitize;
pub mod script;
pub mod ui;

use crate::Scene;

// Re-export the per-component defaults so callers (editor add-menu, `Scene.AddComponent`
// API) reach them through this one entry point; they live in `authoring_defaults`
// only to keep this module under the size cap.
pub use crate::authoring::defaults::{
    attach_default_material, default_animator, default_camera, default_collider, default_light,
    default_material, default_nav_agent, default_post_processing, default_rigidbody,
    material_asset_from_import,
};
pub use crate::authoring::primitive::{
    primitive_collider, primitive_geometry, primitive_mesh_component, primitive_revision,
    subdivided_geometry, Primitive,
};
pub use crate::authoring::script::add_script;

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

// The Add/Remove-Component verbs + their `ComponentKind` enum live in
// `authoring_components` (size-cap split); re-exported so existing
// `authoring::{ComponentKind, add_component, remove_component}` paths still resolve.
pub use crate::authoring::components::{add_component, remove_component, ComponentKind};

// The component-dependency mechanism (#359, rusty's `RequireComponent`) lives in
// `authoring::dependency`; re-exported so the editor Add menu, the camera card's
// remove cascade, the inspector's per-frame reconcile, and scene load all reach the
// one enforcement path through this shared authoring entry point.
pub use crate::authoring::dependency::{
    add_with_requirements, enforce_requirements, has_kind, reconcile_requirements,
    remove_with_cascade, satisfy_requirements,
};

// The prefab structural verbs live in `scene::prefab` (#215) and `scene::prefab_link`
// (#216 linked instances + apply/revert), size-cap splits plus the `.prefab` format
// itself; re-exported here so editor and API reach extract / instantiate (unpacked
// and linked) and the apply/revert/list verbs through this one shared authoring entry
// point.
pub use crate::prefab::apply::{
    apply_instance_field_to_source, apply_instance_to_source, revert_instance_field,
};
pub use crate::prefab::link::{
    list_instance_overrides, reimport_instance, revert_instance_overrides,
};

// The asset-instantiate verb (#182) lives in `scene::asset_instance` (size-cap
// split); re-exported here so the editor's model inspector and the `Scene.Instantiate`
// asset branch both spawn imported sub-objects through this one shared entry point.
pub use crate::asset_instance::{instantiate_asset, instantiate_imported};
pub use crate::model_instance::instantiate_model;

// The per-field material-mutation ops (#287) live in `material`; the editor card and
// the Lua `Material.*` API are siblings over them, so the field write + validation
// (e.g. the alpha clamp) lives once. `ensure_material_key` is the shared resolve-or-
// create the API uses; callers reach the ops via `authoring::material::*`.

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
