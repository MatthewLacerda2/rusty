//! Stop restoring the Play snapshot rewinds the World under every cursor, so a
//! cursor opened during Play reads `Rebuild` (#1705).

use crate::snapshot::SceneSnapshot;
use rusty_components::TransformComponent;
use rusty_ecs::Changes;
use rusty_scene::Scene;

#[test]
fn stop_restoring_the_play_snapshot_reads_rebuild() {
    let mut scene = Scene::new();
    let id = scene.add_entity("e".into());
    let snapshot = SceneSnapshot::capture(&scene); // Play
    let mut cursor = scene.world.change_cursor::<TransformComponent>();
    scene.world.transform_mut(id).expect("live").position.x = 3.0;
    snapshot.restore(&mut scene); // Stop
    assert_eq!(scene.world.changes(&mut cursor), Changes::Rebuild);
    assert_eq!(scene.world.changes(&mut cursor), Changes::Ids(vec![]));
}

mod geometry {
    //! Stop takes each mesh Play left as it was back from the World Play leaves,
    //! copying nothing and looking each file up once, and rebuilds every other one
    //! from its reference.

    use glam::Vec3;

    use crate::asset_instance::instantiate_asset;
    use crate::snapshot::SceneSnapshot;
    use rusty_asset::import_cache::lookups;
    use rusty_components::MeshComponent;
    use rusty_scene::create::{create_entity, Primitive};
    use rusty_scene::Scene;
    use rusty_test_support::models::{quad_file, quad_mode_gltf};

    /// `n` instances of a quad in folder `dir`, and a Box; the quad's path.
    fn level(dir: &str, n: usize) -> (Scene, Vec<u32>, String) {
        let path = quad_file(dir);
        let reference = format!("{path}::Quad");
        let mut scene = Scene::new();
        let mut ids: Vec<u32> = (0..n)
            .map(|_| instantiate_asset(&mut scene, &reference, None, Vec3::ZERO).unwrap())
            .collect();
        ids.push(create_entity(&mut scene, "Box", Some(Primitive::Box)));
        (scene, ids, path)
    }

    fn drawn(scene: &Scene, id: u32) -> (Vec<[f32; 3]>, Vec<u32>) {
        let mesh = scene.world.mesh(id).unwrap();
        (
            mesh.vertices.iter().map(|v| v.position).collect(),
            mesh.indices.to_vec(),
        )
    }

    #[test]
    fn stop_after_an_untouched_play_looks_each_file_up_once_and_copies_nothing() {
        let (mut scene, ids, _) = level("rusty_stop_untouched", 30);
        let edit: Vec<_> = ids.iter().map(|&id| drawn(&scene, id)).collect();
        let revisions: Vec<_> = ids
            .iter()
            .map(|&id| scene.world.mesh(id).unwrap().revision)
            .collect();
        let snapshot = SceneSnapshot::capture(&scene); // Play
        let (before, rebuilt) = (lookups(), rusty_scene::rehydrate::rebuilds());
        snapshot.restore(&mut scene); // Stop
        assert_eq!(lookups(), before + 1, "the quad's file, once, to check it");
        let rebuilt = rusty_scene::rehydrate::rebuilds() - rebuilt;
        assert_eq!(rebuilt, 0, "no copy of a mesh Play left as it was");
        for (i, &id) in ids.iter().enumerate() {
            assert_eq!(drawn(&scene, id), edit[i], "entity {id}");
            assert_eq!(scene.world.mesh(id).unwrap().revision, revisions[i]);
        }
    }

    #[test]
    fn stop_rebuilds_a_mesh_play_replaced() {
        let (mut scene, ids, _) = level("rusty_stop_replaced", 2);
        let (quad, re_meshed, boxed) = (ids[0], ids[1], ids[2]);
        let edit: Vec<_> = ids.iter().map(|&id| drawn(&scene, id)).collect();
        let snapshot = SceneSnapshot::capture(&scene); // Play
        let sphere = create_entity(&mut scene, "S", Some(Primitive::Sphere));
        let sphere = scene.world.mesh(sphere).unwrap().clone();
        scene.world.set_mesh(quad, Some(sphere)); // another primitive
                                                  // A script-built mesh over the reference: the same source, a new revision.
        let built = MeshComponent::new("Asset", Vec::new(), vec![0, 0, 0]);
        let mut live = scene.world.mesh_mut(re_meshed).unwrap();
        (live.vertices, live.indices) = (built.vertices, built.indices);
        live.revision = rusty_asset::MeshRevision::next();
        drop(live);
        scene.world.set_mesh(boxed, None); // removed
        let before = lookups();
        snapshot.restore(&mut scene); // Stop
        assert_eq!(lookups(), before + 1, "the quad's file, once for both");
        for (i, &id) in ids.iter().enumerate() {
            assert_eq!(drawn(&scene, id), edit[i], "entity {id}: the authored mesh");
        }
    }

    #[test]
    fn stop_rebuilds_a_model_written_during_play() {
        let (mut scene, ids, path) = level("rusty_stop_written", 2);
        assert_eq!(drawn(&scene, ids[0]).1.len(), 6, "two triangles");
        let snapshot = SceneSnapshot::capture(&scene); // Play
        let mut follow = crate::rehydrate::model_writes::ModelWrites::default();
        let (json, bin) = quad_mode_gltf(4, &[0, 1, 2]);
        let dir = std::path::Path::new(&path).parent().unwrap();
        std::fs::write(dir.join("quad.bin"), bin).unwrap();
        rusty_core::fs::written::write(std::path::Path::new(&path), json.as_bytes()).unwrap();
        snapshot.restore(&mut scene); // Stop
        for &id in &ids[..2] {
            assert_eq!(
                drawn(&scene, id).1,
                vec![0, 1, 2],
                "entity {id}: the new export"
            );
        }
        assert_eq!(follow.follow(&mut scene), 0, "already read: not read twice");
    }

    /// The model missing when Play began, so every copy drew nothing, and put back
    /// during Play: Stop's lookup retries the import.
    #[test]
    fn stop_retries_an_import_that_failed_at_play_entry() {
        let (mut scene, ids, path) = level("rusty_stop_retried", 2);
        let gltf = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        let data = crate::serialize::to_scene_data(&scene);
        crate::serialize::apply_scene_data(&mut scene, data); // a load without it
        assert!(drawn(&scene, ids[0]).1.is_empty(), "the file is gone");
        let snapshot = SceneSnapshot::capture(&scene); // Play
        std::fs::write(&path, gltf).unwrap();
        snapshot.restore(&mut scene); // Stop
        for &id in &ids[..2] {
            assert_eq!(drawn(&scene, id).1.len(), 6, "entity {id}: imported again");
        }
    }
}
