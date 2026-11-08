//! src/scene/physics_material/collider_tests.rs — a collider's asset path through
//! every way in (#1878): a scene load that clamps an inline value, `Component.Set`,
//! and the path's stored form.

use serde_json::{json, Map};

use super::*;
use crate::scene::authoring::fields::{find, Skip};
use crate::scene::{apply_scene_data, to_scene_data, Scene};

fn asset(name: &str) -> String {
    let path = crate::test_temp::dir().join(name);
    key(&path.to_string_lossy())
}

fn scene_with(collider: ColliderComponent) -> (Scene, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Box".to_string());
    scene.world.set_collider(id, Some(collider));
    (scene, id)
}

#[test]
fn a_load_that_clamps_an_inline_value_keeps_the_asset() {
    let wood = asset("held_wood.physmat");
    write(&wood, PhysicsMaterial::new(0.6, 0.0)).unwrap();
    let collider = ColliderComponent {
        material_asset: Some(wood.clone()),
        ..crate::scene::authoring::default_collider()
    };
    let (scene, id) = scene_with(collider);
    let mut doc = serde_json::to_value(to_scene_data(&scene)).unwrap();
    let entity = doc["entities"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|e| e["id"] == id)
        .unwrap();
    entity["collider"]["material"]["friction"] = json!(-5.0);
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, serde_json::from_value(doc).unwrap());
    let c = loaded.world.collider(id).unwrap().clone();
    assert_eq!(c.material.friction, 0.0, "the inline value is held");
    assert_eq!(c.material_asset, Some(wood.clone()), "the asset is kept");
    let saved = serde_json::to_value(to_scene_data(&loaded)).unwrap();
    assert!(saved.to_string().contains(&wood), "the next save writes it");
}

#[test]
fn one_file_is_one_key_however_it_is_written() {
    let path = asset("keyed.physmat");
    write(&format!("{path} "), PhysicsMaterial::new(0.3, 0.0)).unwrap();
    let mut c = crate::scene::authoring::default_collider();
    let set = crate::scene::authoring::physics::collider::set_material_asset;
    set(&mut c, Some(format!(" {path}"))).unwrap();
    assert_eq!(c.material_asset.as_deref(), Some(path.as_str()));
    assert_eq!(named(&c).as_deref(), Some(path.as_str()));
    set(&mut c, Some(String::new())).unwrap();
    assert_eq!(c.material_asset, None, "an empty path is inline values");
}

#[test]
fn component_set_refuses_a_path_that_does_not_load_and_takes_one_that_does() {
    let (mut scene, id) = scene_with(crate::scene::authoring::default_collider());
    let set = |scene: &mut Scene, path: &str| {
        let changes: Map<_, _> = [("material_asset".to_string(), json!(path))]
            .into_iter()
            .collect();
        (find("Collider").unwrap().write)(&mut scene.world, id, &changes, Skip::Held)
    };
    let e = set(&mut scene, &asset("nowhere.physmat")).unwrap_err();
    assert!(e.contains("no physics material"), "{e}");
    assert_eq!(scene.world.collider(id).unwrap().material_asset, None);
    let ice = asset("set_ice.physmat");
    write(&ice, PhysicsMaterial::new(0.05, 0.0)).unwrap();
    set(&mut scene, &ice).unwrap();
    assert_eq!(scene.world.collider(id).unwrap().material_asset, Some(ice));
}
