//! References and `::` (#1295): a sub-object whose own name holds `::` instantiates,
//! saves, reloads and keeps its mesh; a model under a folder whose name holds `::`
//! is refused when instantiated, rather than spawned with a mesh that loads empty.

use glam::Vec3;

use super::instantiate_asset;
use super::tests::{write_gltf, NO_MATERIAL_GLTF};
use crate::scene::{apply_scene_data, to_scene_data, Scene};

#[test]
fn colon_named_sub_object_survives_save_and_reload() {
    let body = NO_MATERIAL_GLTF.replace(r#""name": "Bare""#, r#""name": "Body::Mesh""#);
    let path = write_gltf("rusty_1295_colons.gltf", &body);
    let reference = format!("{path}::Body::Mesh");
    let mut scene = Scene::new();
    let id = instantiate_asset(&mut scene, &reference, None, Vec3::ZERO).unwrap();
    assert_eq!(*scene.world.name(id).unwrap(), "Body::Mesh");

    let mut restored = Scene::new();
    apply_scene_data(&mut restored, to_scene_data(&scene));
    let mesh = restored.world.mesh(id).expect("mesh kept on reload");
    assert_eq!(mesh.asset_ref.as_deref(), Some(reference.as_str()));
    assert!(!mesh.vertices.is_empty(), "geometry re-imported");
}

#[test]
fn model_under_a_colon_folder_is_refused_with_the_rule() {
    // The refusal comes before any file access, so the folder is only named, never
    // made: Windows can't create one holding `:`.
    let real = write_gltf("rusty_1295_kit.gltf", NO_MATERIAL_GLTF);
    let asset = crate::asset::imported(std::path::Path::new(&real)).unwrap();
    let path = "my::kit/crate.gltf".to_string();
    let mut scene = Scene::new();
    let before = scene.world.collect_entities().len();

    let err = super::instantiate_imported(&mut scene, &path, &asset, "Bare", None, Vec3::ZERO)
        .expect_err("a path holding `::` can't be referenced");
    assert!(err.contains(&path), "names the path: {err}");
    assert!(
        err.contains("may not contain `::`"),
        "names the rule: {err}"
    );
    assert_eq!(
        scene.world.collect_entities().len(),
        before,
        "nothing spawned"
    );
}
