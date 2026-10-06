//! Re-importing a model (#831, #891) rebuilds only that model's unskinned meshes,
//! and keeps the old geometry when the model yields nothing.

use glam::Vec3;

use super::reimport_model;
use crate::asset::SkinData;
use crate::scene::asset_instance::instantiate_asset;
use crate::scene::asset_instance::lightmap_test_support::quad_model;
use crate::scene::Scene;

fn spawn(scene: &mut Scene, path: &str) -> u32 {
    instantiate_asset(scene, &format!("{path}::Quad"), None, Vec3::ZERO).unwrap()
}

fn vertex_count(scene: &Scene, id: u32) -> usize {
    scene.world.mesh(id).unwrap().vertices.len()
}

#[test]
fn a_reimport_rebuilds_only_that_models_unskinned_meshes() {
    let (a, b) = (
        quad_model("rusty_reimport_a"),
        quad_model("rusty_reimport_b"),
    );
    let mut scene = Scene::new();
    let ids = [
        spawn(&mut scene, &a),
        spawn(&mut scene, &b),
        spawn(&mut scene, &a),
    ];
    scene.world.mesh_mut(ids[2]).unwrap().skin = Some(SkinData::default());
    for id in ids {
        scene.world.mesh_mut(id).unwrap().vertices.clear();
    }

    assert_eq!(reimport_model(&mut scene, &a), 1);
    let counts = ids.map(|id| vertex_count(&scene, id));
    assert!(counts[0] > 0, "model A's mesh is rebuilt");
    assert_eq!(
        (counts[1], counts[2]),
        (0, 0),
        "model B and the skinned mesh are not"
    );

    std::fs::remove_file(&a).unwrap();
    assert_eq!(
        reimport_model(&mut scene, &a),
        0,
        "a model that has gone rebuilds none"
    );
    assert_eq!(
        vertex_count(&scene, ids[0]),
        counts[0],
        "and keeps the old geometry"
    );
}
