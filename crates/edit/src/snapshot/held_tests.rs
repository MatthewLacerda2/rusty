use glam::Vec3;

use crate::asset_instance::instantiate_asset;
use crate::serialize::{apply_scene_data, to_scene_data};
use crate::snapshot::SceneSnapshot;
use rusty_components::MeshComponent;
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::Scene;
use rusty_test_support::logs;
use rusty_test_support::models::quad_file;

const OUT_OF_RANGE: f32 = 1.0e9;

fn held(said: &[String]) -> (f32, bool) {
    let (_, hi) = MeshComponent::SCALE_IN_LIGHTMAP.into_inner();
    (hi, said.iter().any(|w| w.contains("scale_in_lightmap")))
}

#[test]
fn stop_holds_an_out_of_range_field_on_a_kept_mesh() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Box", Some(Primitive::Box));
    scene.world.mesh_mut(id).unwrap().scale_in_lightmap = OUT_OF_RANGE;
    let revision = scene.world.mesh(id).unwrap().revision;
    let snapshot = SceneSnapshot::capture(&scene);
    let said = logs::during(|| snapshot.restore(&mut scene));
    let mesh = scene.world.mesh(id).unwrap();
    assert_eq!(mesh.revision, revision, "the live geometry, kept");
    let (hi, warned) = held(&said);
    assert_eq!(mesh.scale_in_lightmap, hi, "held to its range");
    assert!(warned, "{said:?}");
}

#[test]
fn a_load_holds_an_out_of_range_override_on_a_linked_part() {
    let path = quad_file("rusty_load_linked_held");
    let mut scene = Scene::new();
    let root = instantiate_asset(&mut scene, &path, None, Vec3::ZERO).unwrap();
    let part = scene.world.children(root)[0];
    scene.world.mesh_mut(part).unwrap().scale_in_lightmap = OUT_OF_RANGE;
    let data = to_scene_data(&scene);
    let mut loaded = Scene::new();
    let said = logs::during(|| apply_scene_data(&mut loaded, data));
    let (hi, warned) = held(&said);
    assert_eq!(loaded.world.mesh(part).unwrap().scale_in_lightmap, hi);
    assert!(warned, "{said:?}");
}
