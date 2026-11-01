//! A Mesh-shaped collider's shared mesh (#1657) follows its model: a live re-import
//! (the Scale Factor) rebuilds it and refits the collider's box, and a load holds
//! its subdivisions like a Mesh's.

use glam::Vec3;

use crate::components::{ColliderComponent, ColliderShape};
use crate::scene::asset_instance::lightmap_test_support::quad_model;
use crate::scene::asset_instance::{instantiate_asset, set_model_scale_factor};
use crate::scene::authoring::{create_entity, remove_component, ComponentKind, Primitive};
use crate::scene::{apply_scene_data, to_scene_data, Scene};

fn mesh_box(collider: &ColliderComponent) -> (Vec3, Vec3) {
    match collider.shape {
        ColliderShape::Mesh {
            local_min,
            local_max,
            ..
        } => (local_min, local_max),
        _ => panic!("a Mesh-shaped collider"),
    }
}

fn shared_max(scene: &Scene, id: u32) -> Vec3 {
    let collider = scene.world.collider(id).unwrap();
    let shared = collider.shared_mesh.as_ref().expect("a shared mesh");
    let positions = shared.positions.iter().map(|&p| Vec3::from_array(p));
    positions.fold(Vec3::NEG_INFINITY, Vec3::max)
}

#[test]
fn a_scale_factor_change_reaches_a_part_without_its_mesh() {
    let path = quad_model("rusty_reimport_shared");
    let mut scene = Scene::new();
    let id = instantiate_asset(&mut scene, &format!("{path}::Quad"), None, Vec3::ZERO);
    let id = id.unwrap();
    let collider = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ONE);
    scene.world.set_collider(id, Some(collider));
    assert!(remove_component(&mut scene, id, ComponentKind::Mesh));
    assert_eq!(shared_max(&scene, id), Vec3::new(1.0, 1.0, 0.0));

    // It counts Meshes re-imported (`Assets.md`), and the part has none.
    assert_eq!(set_model_scale_factor(&mut scene, &path, 0.5), Ok(0));
    assert!(shared_max(&scene, id).abs_diff_eq(Vec3::new(0.5, 0.5, 0.0), 1e-6));
    let (_, max) = mesh_box(&scene.world.collider(id).unwrap());
    assert!(
        max.abs_diff_eq(Vec3::new(0.5, 0.5, 0.0), 1e-6),
        "refit: {max}"
    );
    assert!(
        scene.world.collider(id).unwrap().aabb_max.x < 0.6,
        "the world bounds follow"
    );
}

#[test]
fn a_re_import_with_no_geometry_keeps_the_old_shared_mesh() {
    let path = quad_model("rusty_reimport_shared_empty");
    let mut scene = Scene::new();
    let id = instantiate_asset(&mut scene, &format!("{path}::Quad"), None, Vec3::ZERO);
    let id = id.unwrap();
    let collider = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ONE);
    scene.world.set_collider(id, Some(collider));
    assert!(remove_component(&mut scene, id, ComponentKind::Mesh));
    // The quad is now points: not a surface, so it imports no triangles.
    let (json, bin) = crate::asset::fixtures::quad_mode_gltf(0, &[]);
    let dir = std::path::Path::new(&path).parent().unwrap();
    std::fs::write(dir.join("quad.bin"), bin).unwrap();
    std::fs::write(&path, json).unwrap();

    assert_eq!(set_model_scale_factor(&mut scene, &path, 0.5), Ok(0));
    assert_eq!(shared_max(&scene, id), Vec3::new(1.0, 1.0, 0.0), "kept");
    let (_, max) = mesh_box(&scene.world.collider(id).unwrap());
    assert_eq!(
        max,
        Vec3::ONE,
        "the box is left as it was, not refit to nothing"
    );
}

#[test]
fn a_loaded_shared_mesh_holds_its_subdivisions() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Blockout", Some(Primitive::Box));
    let collider = ColliderComponent::from_mesh_bounds(true, Vec3::splat(-0.5), Vec3::splat(0.5));
    scene.world.set_collider(id, Some(collider));
    remove_component(&mut scene, id, ComponentKind::Mesh);
    let text = serde_json::to_string(&to_scene_data(&scene)).unwrap();
    let text = text.replace(
        "\"primitive_type\":\"Box\"}",
        "\"primitive_type\":\"Box\",\"subdivisions\":9}",
    );
    assert!(text.contains("\"subdivisions\":9"), "{text}");
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, serde_json::from_str(&text).unwrap());
    let collider = loaded.world.collider(id).unwrap();
    let shared = collider.shared_mesh.as_ref().unwrap();
    assert_eq!(shared.subdivisions, 1, "a Box takes no subdivisions");
    assert!(!shared.positions.is_empty());
}
