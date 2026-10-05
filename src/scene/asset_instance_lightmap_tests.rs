//! Generate Lightmap UVs from the scene's side (#831): ticking it re-imports the
//! model's meshes already in the scene, through the one verb the inspector and the
//! `Assets` API share (its Lua side: `scripting::tests_assets`).

use glam::Vec3;

use super::{instantiate_asset, set_lightmap_uv_settings};
use crate::asset::fixtures::quad_mode_gltf;
use crate::asset::LightmapUvSettings;
use crate::scene::Scene;

/// A one-quad glTF (no `TEXCOORD_1`) in its own temp dir, with no sidecar yet.
pub(crate) fn quad_model(dir: &str) -> String {
    let dir = std::env::temp_dir().join(dir);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (json, bin) = quad_mode_gltf(4, &[0, 1, 2, 0, 2, 3]);
    std::fs::write(dir.join("quad.bin"), bin).unwrap();
    let path = dir.join("quad.gltf");
    std::fs::write(&path, json).unwrap();
    path.to_string_lossy().into_owned()
}

pub(crate) fn has_lightmap_uv(scene: &Scene, id: u32) -> bool {
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
