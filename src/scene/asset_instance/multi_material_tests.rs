//! Instancing a multi-material model (#1285): the entity carries one material per
//! run of its mesh, and a saved scene keeps them all.

use glam::Vec3;

use super::instantiate_asset;
use crate::scene::{apply_scene_data, to_scene_data, Scene, SceneData};

/// A scene holding the FBX fixture's two-material quad, and the quad's id.
fn quad() -> (Scene, u32) {
    let path = crate::test_temp::dir().join("quad.fbx");
    let fixture = include_str!("../../../tests/fixtures/models/embedded_textures.fbx");
    std::fs::write(&path, fixture).unwrap();
    let reference = format!("{}::Quad", path.to_string_lossy());
    let mut scene = Scene::new();
    let id = instantiate_asset(&mut scene, &reference, None, Vec3::ZERO).unwrap();
    (scene, id)
}

#[test]
fn an_instanced_quad_references_one_material_per_run() {
    let (scene, id) = quad();
    let keys = scene.world.material(id).unwrap().materials.clone();
    let names: Vec<_> = keys
        .iter()
        .map(|k| k.rsplit("::").next().unwrap())
        .collect();
    assert_eq!(names, ["Red", "Blue"]);
    assert!(
        keys.iter().all(|k| scene.materials.contains_key(k)),
        "{keys:?}"
    );
    assert_eq!(scene.world.mesh(id).unwrap().ranges, [0..3, 3..6]);
}

#[test]
fn a_saved_scene_keeps_every_material_and_reloads_the_runs() {
    let (scene, id) = quad();
    let json = serde_json::to_string(&to_scene_data(&scene)).unwrap();
    let data: SceneData = serde_json::from_str(&json).unwrap();
    let mut restored = Scene::new();
    apply_scene_data(&mut restored, data);
    let before = &scene.world.material(id).unwrap().materials.clone();
    assert_eq!(&restored.world.material(id).unwrap().materials, before);
    assert!(before.iter().all(|k| restored.materials.contains_key(k)));
    assert_eq!(restored.world.mesh(id).unwrap().ranges, [0..3, 3..6]);
    let again = serde_json::to_string(&to_scene_data(&restored)).unwrap();
    assert_eq!(again, json);
}
