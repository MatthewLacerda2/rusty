//! The scene mesh library's users (#2059's review): a bad entry on load, many
//! entities sharing one build, a Mesh collider's kept copy, a prefab rebuild and
//! `delete_mesh`.

use super::{missing_key, BUILDS};
use crate::authoring::{create_entity, Primitive};
use crate::{apply_scene_data, to_scene_data, Scene};
use rusty_components::mesh::MeshData;

fn triangle(size: f32) -> MeshData {
    MeshData {
        positions: vec![[0.0; 3], [size, 0.0, 0.0], [0.0, size, 0.0]],
        indices: vec![0, 1, 2],
        ..Default::default()
    }
}

use super::delete_tests::kept_user;

fn kept_len(scene: &Scene, id: u32) -> usize {
    let collider = scene.world.collider(id).unwrap();
    collider
        .shared_mesh
        .as_ref()
        .map_or(0, |k| k.positions.len())
}

#[test]
fn a_file_with_bad_entries_loads_without_a_panic_and_warns() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Piece".into());
    scene.define_mesh("good", triangle(1.0)).unwrap();
    scene.set_shared_mesh(id, "good").unwrap();
    let mut data = to_scene_data(&scene);
    let mut bad = |name: &str, edit: fn(&mut MeshData)| {
        let mut mesh = triangle(1.0);
        edit(&mut mesh);
        data.meshes.insert(name.into(), mesh.into());
    };
    bad("past", |m| m.indices[2] = 3);
    bad("short_normals", |m| m.normals = vec![[0.0, 1.0, 0.0]]);
    bad("short_uvs", |m| m.uvs = vec![[0.0; 2]; 2]);
    bad("Box", |_| ());
    for entity in &mut data.entities {
        entity.mesh.as_mut().unwrap().asset_ref = Some("past".into());
    }
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, data);
    assert_eq!(loaded.meshes.keys().collect::<Vec<_>>(), ["good"]);
    for name in ["past", "short_normals", "short_uvs", "Box"] {
        assert!(crate::rehydrate::warned_of(&missing_key(name)), "{name}");
    }
    assert!(
        loaded.world.mesh(id).unwrap().vertices.is_empty(),
        "draws nothing"
    );
}

#[test]
fn one_mesh_on_many_entities_builds_once_and_touches_no_other_user() {
    let mut scene = Scene::new();
    scene.define_mesh("gib", triangle(1.0)).unwrap();
    let first = scene.add_entity("Gib".into());
    scene.set_shared_mesh(first, "gib").unwrap();
    let revision = scene.world.mesh(first).unwrap().revision;
    let built = BUILDS.with(|n| n.get());
    for i in 0..50 {
        let id = scene.add_entity(format!("Gib{i}"));
        scene.set_shared_mesh(id, "gib").unwrap();
        assert_eq!(scene.world.mesh(id).unwrap().vertices.len(), 3);
        assert_eq!(scene.world.mesh(id).unwrap().revision, revision);
    }
    assert_eq!(
        BUILDS.with(|n| n.get()),
        built,
        "built once, before the loop"
    );
    assert_eq!(scene.world.mesh(first).unwrap().revision, revision);
    assert_eq!(revision, scene.meshes["gib"].revision);
}

#[test]
fn a_kept_copy_follows_a_replace_an_undo_and_a_play_load() {
    let mut scene = Scene::new();
    scene.define_mesh("plank", triangle(1.0)).unwrap();
    let id = kept_user(&mut scene, "plank");
    assert_eq!(kept_len(&scene, id), 3);
    scene.begin_edits();
    let mut quad = triangle(2.0);
    quad.positions.push([2.0, 2.0, 0.0]);
    quad.indices.extend([1, 3, 2]);
    scene.define_mesh("plank", quad).unwrap();
    scene.commit_edits(Some("Replace"));
    assert_eq!(kept_len(&scene, id), 4, "a replace reaches a kept copy");
    scene.undo();
    assert_eq!(kept_len(&scene, id), 3, "and so does its Undo");
    let survivors = std::collections::BTreeSet::from([id]);
    let empty = to_scene_data(&Scene::new());
    scene.swap_in(empty, "next.scene", &survivors).unwrap();
    assert!(scene.meshes.contains_key("plank"), "a Play load carries it");
    assert_eq!(kept_len(&scene, id), 3);
}

#[test]
fn a_rebuilt_prefab_instance_keeps_drawing_its_scene_mesh() {
    let mut source = Scene::new();
    let root = create_entity(&mut source, "Root", Some(Primitive::Box));
    let child = create_entity(&mut source, "Child", Some(Primitive::Sphere));
    source.set_parent(child, Some(root)).unwrap();
    let path = rusty_test_support::temp::dir().join("piece_2059.prefab");
    let path = path.to_string_lossy().into_owned();
    crate::save_prefab(&source, root, &path).unwrap();
    let mut scene = Scene::new();
    let root = crate::load_and_instantiate_linked(&mut scene, &path, None).unwrap();
    let child = scene.world.children(root)[0];
    scene.define_mesh("piece", triangle(1.0)).unwrap();
    scene.set_shared_mesh(child, "piece").unwrap();
    crate::prefab::link::reimport_instance(&mut scene, root).unwrap();
    let mesh = scene.world.mesh(child).unwrap();
    assert_eq!(mesh.asset_ref.as_deref(), Some("piece"));
    assert_eq!(mesh.vertices.len(), 3, "the override keeps its geometry");
}
