//! Unit tests for the structured scene-read (#180). Fixtures are built with the
//! same authoring verbs the API exposes, so the read reflects what create/add did.

use super::*;
use crate::scene::authoring::{add_component, create_entity, ComponentKind, Primitive};
use crate::scene::Camera;
use crate::scene::Scene;

/// The UI screen size the fixtures are snapshotted on.
const SCREEN: Vec2 = Vec2::new(1920.0, 1080.0);

fn cam() -> Camera {
    Camera::new(Vec3::new(1.0, 2.0, 3.0), 10.0, -5.0)
}

#[test]
fn world_envelope_reports_frame_camera_and_play_state() {
    let scene = Scene::new();
    let v = world_value(&scene, &cam(), 42, true, SCREEN);
    assert_eq!(v["frame"].as_u64(), Some(42));
    assert_eq!(v["play_state"].as_str(), Some("playing"));
    assert_eq!(v["camera"]["pos"][0].as_f64(), Some(1.0));
    assert!(v["entities"].as_array().unwrap().is_empty());

    let editor = world_value(&scene, &cam(), 0, false, SCREEN);
    assert_eq!(editor["play_state"].as_str(), Some("editor"));
}

#[test]
fn entity_reports_transform_scale_and_component_inventory() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Crate", Some(Primitive::Box));
    add_component(&mut scene, id, ComponentKind::Light);
    if let Some(mut t) = scene.world.transform_mut(id) {
        t.scale = Vec3::new(2.0, 3.0, 4.0);
    }

    let v = world_value(&scene, &cam(), 0, false, SCREEN);
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
    // Edit the entity's referenced library material (the data is shared, not inline).
    let key = scene.world.material(id).unwrap().material.clone();
    if let Some(mat) = scene.materials.get_mut(&key) {
        mat.base_color = [0.5, 0.25, 0.125];
        mat.base_color_map = Some("project/textures/wood.png".to_string());
    }

    let ent = &world_value(&scene, &cam(), 0, false, SCREEN)["entities"][0];
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

    let ent = &world_value(&scene, &cam(), 0, false, SCREEN)["entities"][0];
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
    let direct = entity_value(&scene, id, wm, &crate::ui::UiView::screen(SCREEN));
    assert_eq!(direct["name"].as_str(), Some("Solo"));
    // A transform-only entity has no mesh/collider, so no bounds.
    assert!(direct["bounds"].is_null());
}

#[test]
fn ui_elements_report_their_components_and_computed_rect() {
    let mut scene = Scene::new();
    let root = create_entity(&mut scene, "Canvas", None);
    add_component(&mut scene, root, ComponentKind::Canvas);
    let hud = create_entity(&mut scene, "Hud", None);
    add_component(&mut scene, hud, ComponentKind::RectTransform);
    scene.set_parent(hud, Some(root)).expect("parent exists");
    let v = world_value(&scene, &cam(), 0, false, SCREEN);
    let ent = &v["entities"][1];
    assert_eq!(ent["components"], json!(["RectTransform"]));
    assert_eq!(ent["rect_transform"]["size_delta"], json!([100.0, 100.0]));
    assert_eq!(ent["ui_rect"]["x"], json!(910.0));
    assert_eq!(ent["ui_rect"]["screen"]["width"], json!(100.0));
    assert_eq!(ent["ui_rect"]["canvas"], json!(root));
    assert_eq!(
        v["entities"][0]["canvas"]["reference_resolution"],
        json!([1920.0, 1080.0])
    );
    assert!(v["entities"][0]["ui_rect"].is_object());
}

#[test]
fn nav_agent_reports_its_turning() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Bot", None);
    add_component(&mut scene, id, ComponentKind::NavMeshAgent);
    let ent = &world_value(&scene, &cam(), 0, false, SCREEN)["entities"][0];
    let agent = &ent["nav_agent"];
    assert_eq!(agent["update_rotation"].as_bool(), Some(true));
    assert_eq!(agent["angular_speed"].as_f64(), Some(120.0));
    assert_eq!(agent["angular_acceleration"].as_f64(), Some(720.0));
}
