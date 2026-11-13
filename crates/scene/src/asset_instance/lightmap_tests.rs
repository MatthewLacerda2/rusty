//! Generate Lightmap UVs from the scene's side (#831): ticking it re-imports the
//! model's meshes already in the scene, through the one verb the inspector and the
//! `Assets` API share (its Lua side: `scripting::tests::assets`).

use glam::Vec3;

use super::{instantiate_asset, set_lightmap_uv_settings};
use crate::Scene;
use rusty_asset::LightmapUvSettings;
use rusty_test_support::models::quad_file as quad_model;

fn has_lightmap_uv(scene: &Scene, id: u32) -> bool {
    let mesh = scene.world.mesh(id).unwrap();
    mesh.vertices.iter().any(|v| v.lightmap_uv != [0.0, 0.0])
}

#[test]
fn ticking_the_box_reimports_the_models_meshes_in_the_scene() {
    let path = quad_model("rusty_lmuv_verb");
    let mut scene = Scene::new();
    let id = instantiate_asset(&mut scene, &format!("{path}::Quad"), None, Vec3::ZERO);
    let id = id.unwrap();
    assert!(!has_lightmap_uv(&scene, id), "off by default");
    let on = LightmapUvSettings {
        generate: true,
        ..Default::default()
    };
    assert_eq!(set_lightmap_uv_settings(&mut scene, &path, on), Ok(1));
    assert!(has_lightmap_uv(&scene, id));
    let off = LightmapUvSettings::default();
    assert_eq!(set_lightmap_uv_settings(&mut scene, &path, off), Ok(1));
    assert!(!has_lightmap_uv(&scene, id));
}
