//! The Scale Factor from the scene's side (#1398): setting it re-imports the model's
//! meshes already in the scene, a skinned one with its bones moved to the scaled
//! rest pose (its Lua side: `scripting::tests::assets`).

use glam::Vec3;

use super::{instantiate_asset, set_model_scale_factor};
use crate::asset::fixtures::{skinned_buffer, SKINNED_GLTF};
use crate::scene::asset_instance::lightmap_test_support::quad_model;
use crate::scene::Scene;

fn max_corner(scene: &Scene, id: u32) -> Vec3 {
    let mesh = scene.world.mesh(id).unwrap();
    let positions = mesh.vertices.iter().map(|v| Vec3::from_array(v.position));
    positions.fold(Vec3::NEG_INFINITY, Vec3::max)
}

#[test]
fn setting_the_scale_factor_reimports_the_models_meshes_in_the_scene() {
    let path = quad_model("rusty_scale_verb");
    let mut scene = Scene::new();
    let id = instantiate_asset(&mut scene, &format!("{path}::Quad"), None, Vec3::ZERO);
    let id = id.unwrap();
    assert_eq!(max_corner(&scene, id), Vec3::new(1.0, 1.0, 0.0));
    assert_eq!(set_model_scale_factor(&mut scene, &path, 0.01), Ok(1));
    assert!(max_corner(&scene, id).abs_diff_eq(Vec3::new(0.01, 0.01, 0.0), 1e-7));
    assert_eq!(set_model_scale_factor(&mut scene, &path, 1.0), Ok(1));
    assert_eq!(max_corner(&scene, id), Vec3::new(1.0, 1.0, 0.0));
}

#[test]
fn a_skinned_instances_bones_move_to_the_scaled_rest_pose() {
    let dir = crate::test_temp::dir().join("rusty_scale_skinned_verb");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("model.bin"), skinned_buffer()).unwrap();
    let path = dir.join("model.gltf");
    std::fs::write(&path, SKINNED_GLTF).unwrap();
    let path = path.to_string_lossy().into_owned();

    let mut scene = Scene::new();
    let reference = format!("{path}::Skinned");
    let id = instantiate_asset(&mut scene, &reference, None, Vec3::ZERO).unwrap();
    let bones = scene.world.mesh(id).unwrap().skeleton.bones.clone();
    assert_eq!(bones.len(), 2);

    assert_eq!(set_model_scale_factor(&mut scene, &path, 0.01), Ok(1));
    let after = scene.world.mesh(id).unwrap().skeleton.bones.clone();
    assert_eq!(after, bones, "the bones keep their entities");
    let local = |bone: Option<u32>| scene.world.transform(bone.unwrap()).unwrap().position;
    assert!(local(bones[0]).abs_diff_eq(Vec3::new(0.01, 0.0, 0.0), 1e-7));
    assert!(local(bones[1]).abs_diff_eq(Vec3::new(0.0, 0.02, 0.0), 1e-7));
}

#[test]
fn the_change_is_noted_for_other_caches_and_the_scene_reads_it_once() {
    use crate::core::fs::written::{note_written, written_since};
    use crate::scene::rehydrate::model_writes::ModelWrites;
    let path = quad_model("rusty_scale_noted");
    let mut scene = Scene::new();
    instantiate_asset(&mut scene, &format!("{path}::Quad"), None, Vec3::ZERO).unwrap();
    let mut follow = ModelWrites::default();
    // Another model's note waiting, as any test or tool running alongside makes.
    note_written(&quad_model("rusty_scale_noted_other"));
    let (seen, _) = written_since(u64::MAX);
    assert_eq!(set_model_scale_factor(&mut scene, &path, 0.5), Ok(1));
    assert!(
        written_since(seen).1.contains(&path),
        "the particle cache sees it"
    );
    assert_eq!(follow.follow(&mut scene), 0, "already re-imported");
    assert_eq!(follow.follow(&mut scene), 0, "and not again next frame");
}

#[test]
fn a_nan_or_unchanged_factor_changes_nothing() {
    use crate::core::fs::written::written_since;
    let path = quad_model("rusty_scale_unchanged");
    let mut scene = Scene::new();
    instantiate_asset(&mut scene, &format!("{path}::Quad"), None, Vec3::ZERO).unwrap();
    let (seen, _) = written_since(u64::MAX);
    assert_eq!(set_model_scale_factor(&mut scene, &path, f32::NAN), Ok(0));
    assert_eq!(set_model_scale_factor(&mut scene, &path, 1.0), Ok(0));
    assert!(!written_since(seen).1.contains(&path), "no write noted");
}

#[test]
fn a_saved_mesh_colliders_box_follows_a_scale_factor_set_since() {
    use crate::components::{ColliderComponent, ColliderShape};
    use crate::scene::serialize::{apply_scene_data, to_scene_data};
    let path = quad_model("rusty_scale_collider");
    let mut scene = Scene::new();
    let id = instantiate_asset(&mut scene, &format!("{path}::Quad"), None, Vec3::ZERO).unwrap();
    let collider = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ONE);
    scene.world.set_collider(id, Some(collider));
    let data = to_scene_data(&scene);
    crate::asset::sidecar::set_scale(std::path::Path::new(&path), 0.01).unwrap();
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, data);
    let id = loaded.world.ids_with_collider()[0];
    let ColliderShape::Mesh { local_max, .. } = loaded.world.collider(id).unwrap().shape else {
        panic!("a mesh collider");
    };
    assert!(
        local_max.abs_diff_eq(Vec3::new(0.01, 0.01, 0.0), 1e-7),
        "{local_max}"
    );
}
