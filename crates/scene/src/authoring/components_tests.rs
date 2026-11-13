//! Tests for `scene::authoring::components`: the Add/Remove verbs and `ComponentKind`.

use super::*;
use crate::authoring::{create_entity, Primitive};

#[test]
fn add_remove_component_with_cascades() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Mob", None);
    assert!(add_component(&mut scene, id, ComponentKind::Light));
    assert!(scene.world.has_light(id));
    assert!(remove_component(&mut scene, id, ComponentKind::Light));
    assert!(!scene.world.has_light(id));

    // Adding PostProcessing auto-adds its required Camera (RequireComponent).
    add_component(&mut scene, id, ComponentKind::PostProcessing);
    assert!(scene.world.has_post_processing(id) && scene.world.has_camera(id));

    // Removing Camera cascades to PostProcessing (editor parity).
    remove_component(&mut scene, id, ComponentKind::Camera);
    assert!(!scene.world.has_camera(id) && !scene.world.has_post_processing(id));

    assert!(!add_component(&mut scene, 9999, ComponentKind::Light));
}

#[test]
fn add_remove_audio_source() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Speaker", None);
    assert!(add_component(&mut scene, id, ComponentKind::Audio));
    assert!(scene.world.has_audio(id));
    assert!(remove_component(&mut scene, id, ComponentKind::Audio));
    assert!(!scene.world.has_audio(id));
}

#[test]
fn all_lists_every_kind_once_in_declaration_order() {
    // `ALL[i]`'s discriminant is `i`: no gap, duplicate or reorder. Every kind also
    // parses from its own name, so `parse` cannot miss a variant either.
    for (i, &kind) in ComponentKind::ALL.iter().enumerate() {
        assert_eq!(kind as usize, i, "{kind:?} out of place in ALL");
        assert_eq!(ComponentKind::parse(&format!("{kind:?}")), Some(kind));
    }
}

#[test]
fn parse_names_are_case_insensitive() {
    assert_eq!(
        ComponentKind::parse("material"),
        Some(ComponentKind::Texture)
    );
    assert_eq!(ComponentKind::parse("AUDIO"), Some(ComponentKind::Audio));
    assert_eq!(
        ComponentKind::parse("AudioSource"),
        Some(ComponentKind::Audio)
    );
    assert_eq!(ComponentKind::parse("nope"), None);
}

#[test]
fn an_unknown_kind_lists_the_kinds() {
    let err = ComponentKind::parse_or_list("Meshh").unwrap_err();
    assert!(err.starts_with("unknown component 'Meshh' (kinds: AIPerception, Animator,"));
    let listed = err.split("(kinds: ").nth(1).unwrap().trim_end_matches(')');
    // The names Component.* uses, each one AddComponent takes, every kind once.
    assert!(
        listed.contains("Material, ") && !listed.contains("Texture"),
        "{err}"
    );
    let parsed: Vec<_> = listed
        .split(", ")
        .filter_map(ComponentKind::parse)
        .collect();
    assert_eq!(parsed.len(), ComponentKind::ALL.len(), "{err}");
    assert_eq!(
        ComponentKind::parse_or_list("MeshRenderer"),
        Ok(ComponentKind::Mesh)
    );
}

#[test]
fn removing_a_mesh_keeps_the_entity_and_its_collider() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Blockout", Some(Primitive::Box));
    add_component(&mut scene, id, ComponentKind::Collider);
    add_component(&mut scene, id, ComponentKind::Texture);
    assert!(remove_component(&mut scene, id, ComponentKind::Mesh));
    let w = &scene.world;
    assert!(!w.has_mesh(id) && w.has_collider(id), "collider stays");
    assert!(!w.has_material(id), "the renderer's material goes with it");
    // Removing again is a no-op success; a missing entity is not.
    assert!(remove_component(&mut scene, id, ComponentKind::Mesh));
    assert!(!remove_component(&mut scene, 9999, ComponentKind::Mesh));
}

#[test]
fn adding_a_mesh_attaches_the_default_box() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Empty", None);
    assert!(add_component(&mut scene, id, ComponentKind::Mesh));
    let mesh = scene.world.mesh(id).expect("a mesh");
    assert_eq!(mesh.primitive_type, "Box");
    assert!(!mesh.vertices.is_empty());
    assert!(!scene.world.has_material(id), "a Mesh adds no Material");
}
