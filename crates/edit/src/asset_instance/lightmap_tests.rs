use glam::Vec3;

use super::{instantiate_asset, set_lightmap_uv_settings};
use rusty_asset::LightmapUvSettings;
use rusty_scene::Scene;
use rusty_test_support::models::quad_file as quad_model;

#[test]
fn ticking_the_box_reimports_the_models_meshes_in_the_scene() {
    let path = quad_model("rusty_lmuv_verb");
    let mut scene = Scene::new();
    let id = instantiate_asset(&mut scene, &format!("{path}::Quad"), None, Vec3::ZERO);
    let id = id.unwrap();
    assert!(
        !scene.world.mesh(id).unwrap().has_lightmap_uv(),
        "off by default"
    );
    let on = LightmapUvSettings {
        generate: true,
        ..Default::default()
    };
    assert_eq!(set_lightmap_uv_settings(&mut scene, &path, on), Ok(1));
    assert!(scene.world.mesh(id).unwrap().has_lightmap_uv());
    let off = LightmapUvSettings::default();
    assert_eq!(set_lightmap_uv_settings(&mut scene, &path, off), Ok(1));
    assert!(!scene.world.mesh(id).unwrap().has_lightmap_uv());
}
