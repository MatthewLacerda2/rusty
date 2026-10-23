//! A `null` override leaf costs only itself (#1264): destroying the entity an
//! instance's LOD renderer override names writes `null` into a list of ids, and the
//! entity's other overrides still survive a save, a load and a propagation.

use super::*;
use crate::components::{LodGroupComponent, LodLevel};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::prefab::{instantiate_prefab_linked, read_prefab_file};
use crate::scene::{
    apply_scene_data, save_prefab, to_scene_data, MaterialAsset, MaterialComponent, SceneData,
};

/// A Box root whose LOD group renders its Sphere child, saved as a prefab.
fn write_lod_prefab() -> String {
    let mut scene = Scene::new();
    let root = create_entity(&mut scene, "Root", Some(Primitive::Box));
    let child = create_entity(&mut scene, "Child", Some(Primitive::Sphere));
    scene.set_parent(child, Some(root)).unwrap();
    let level = LodLevel {
        renderers: vec![child],
        ..LodLevel::default()
    };
    let lod = LodGroupComponent {
        levels: vec![level],
        ..LodGroupComponent::default()
    };
    scene.world.set_lod_group(root, Some(lod));
    let path = crate::test_temp::dir().join("rusty_1264_lod_override.prefab");
    let path = path.to_string_lossy().into_owned();
    save_prefab(&scene, root, &path).unwrap();
    path
}

fn reload(scene: &Scene) -> Scene {
    let json = serde_json::to_string(&to_scene_data(scene)).unwrap();
    let data: SceneData = serde_json::from_str(&json).unwrap();
    let mut out = Scene::new();
    apply_scene_data(&mut out, data);
    out
}

#[test]
fn a_destroyed_lod_renderer_override_keeps_the_other_overrides() {
    let path = write_lod_prefab();
    let mut scene = Scene::new();
    let post = scene.add_entity("Post".into());
    let prefab = read_prefab_file(&path).unwrap();
    let root = instantiate_prefab_linked(&mut scene, &prefab, None, &path);
    scene.world.transform_mut(root).unwrap().position.x = 3.0;
    scene
        .materials
        .insert("Brick".into(), MaterialAsset::default());
    let brick = MaterialComponent::new("Brick");
    scene.world.set_material(root, Some(brick));
    scene.world.lod_group_mut(root).unwrap().levels[0].renderers = vec![post];
    record_instance_overrides(&mut scene, root).unwrap();

    scene.destroy_entity(post);
    let mut loaded = reload(&scene);
    reimport_instance(&mut loaded, root).unwrap();

    let x = loaded.world.transform(root).unwrap().position.x;
    assert_eq!(x, 3.0, "the transform override survives");
    let material = loaded.world.material(root).unwrap().first().to_string();
    assert_eq!(material, "Brick", "the material override survives");
    let lod = loaded.world.lod_group(root).unwrap().clone();
    assert!(
        lod.levels[0].renderers.is_empty(),
        "the dead renderer is dropped"
    );
    let _ = std::fs::remove_file(&path);
}
