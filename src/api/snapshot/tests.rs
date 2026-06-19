//! Unit tests for the structured scene-read (#180). Fixtures are built with the
//! same authoring verbs the API exposes, so the read reflects what create/add did.

use super::*;
use crate::render::Camera;
use crate::scene::authoring::{add_component, create_entity, ComponentKind, Primitive};
use crate::scene::Scene;

fn cam() -> Camera {
    Camera::new(Vec3::new(1.0, 2.0, 3.0), 10.0, -5.0)
}

#[test]
fn world_envelope_reports_frame_camera_and_play_state() {
    let scene = Scene::new();
    let v = world_value(&scene, &cam(), 42, true);
    assert_eq!(v["frame"].as_u64(), Some(42));
    assert_eq!(v["play_state"].as_str(), Some("playing"));
    assert_eq!(v["camera"]["pos"][0].as_f64(), Some(1.0));
    assert!(v["entities"].as_array().unwrap().is_empty());

    let editor = world_value(&scene, &cam(), 0, false);
    assert_eq!(editor["play_state"].as_str(), Some("editor"));
}

#[test]
fn entity_reports_transform_scale_and_component_inventory() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Crate", Some(Primitive::Box));
    add_component(&mut scene, id, ComponentKind::Light);
    if let Some(mut e) = scene.get_entity_mut(id) {
        e.transform.scale = Vec3::new(2.0, 3.0, 4.0);
    }

    let v = world_value(&scene, &cam(), 0, false);
    let ent = &v["entities"][0];
    assert_eq!(ent["name"].as_str(), Some("Crate"));
    assert_eq!(ent["transform"]["scale"].as_array().unwrap().len(), 3);
    assert_eq!(ent["transform"]["scale"][1].as_f64(), Some(3.0));

    let inv: Vec<&str> = ent["components"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c.as_str())
        .collect();
    assert!(inv.contains(&"Mesh"), "primitive carries a mesh");
    assert!(inv.contains(&"Light"), "added Light shows in the inventory");
}

#[test]
fn mesh_and_material_reflect_authored_values() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Box", Some(Primitive::Box));
    add_component(&mut scene, id, ComponentKind::Texture);
    if let Some(mut e) = scene.get_entity_mut(id) {
        if let Some(tex) = e.texture.as_mut() {
            tex.color = [0.5, 0.25, 0.125];
            tex.path = "project/textures/wood.png".to_string();
        }
    }

    let ent = &world_value(&scene, &cam(), 0, false)["entities"][0];
    assert_eq!(ent["mesh"]["primitive_type"].as_str(), Some("Box"));
    assert_eq!(ent["material"]["color"][0].as_f64(), Some(0.5));
    assert_eq!(
        ent["material"]["texture"].as_str(),
        Some("project/textures/wood.png")
    );
}

#[test]
fn collider_yields_world_bounds() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Solid", Some(Primitive::Box));
    add_component(&mut scene, id, ComponentKind::Collider);
    scene.update_entity_collider(id);

    let ent = &world_value(&scene, &cam(), 0, false)["entities"][0];
    let bounds = &ent["bounds"];
    assert!(bounds.is_object(), "an entity with geometry exposes bounds");
    assert_eq!(bounds["min"].as_array().unwrap().len(), 3);
    assert_eq!(bounds["max"].as_array().unwrap().len(), 3);
}

#[test]
fn entity_value_matches_world_entry() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Solo", None);
    let wm = scene.compute_world_matrix(id);
    let direct = entity_value(&scene.get_entity(id).unwrap(), wm);
    assert_eq!(direct["name"].as_str(), Some("Solo"));
    // A transform-only entity has no mesh/collider, so no bounds.
    assert!(direct["bounds"].is_null());
}
