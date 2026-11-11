//! A library mesh's revisions only move forward (#2059's review): an entry put
//! back by Undo or Stop takes a revision newer than any an entity drew, so the
//! renderer, which keeps the newest upload for a name, uploads its data again.

use crate::components::mesh::MeshData;
use crate::scene::{Scene, SceneSnapshot};

fn triangle(size: f32) -> MeshData {
    MeshData {
        positions: vec![[0.0; 3], [size, 0.0, 0.0], [0.0, size, 0.0]],
        indices: vec![0, 1, 2],
        ..Default::default()
    }
}

/// `edit` as one undo step named `name`.
fn step(scene: &mut Scene, name: &str, edit: impl FnOnce(&mut Scene)) {
    scene.begin_edits();
    edit(scene);
    scene.commit_edits(Some(name));
}

fn revision(scene: &Scene, id: u32) -> crate::components::mesh::MeshRevision {
    scene.world.mesh(id).unwrap().revision
}

#[test]
fn undo_back_to_older_data_gives_the_next_user_a_newer_revision() {
    let mut scene = Scene::new();
    let (f, g) = (scene.add_entity("F".into()), scene.add_entity("G".into()));
    step(&mut scene, "A", |s| {
        s.define_mesh("m", triangle(1.0)).unwrap()
    });
    step(&mut scene, "B", |s| {
        s.define_mesh("m", triangle(2.0)).unwrap()
    });
    step(&mut scene, "Use", |s| s.set_shared_mesh(f, "m").unwrap());
    let uploaded = revision(&scene, f);
    assert_eq!(scene.undo().as_deref(), Some("Use"));
    assert_eq!(scene.undo().as_deref(), Some("B"));
    assert!(scene.mesh_users("m").is_empty());
    scene.set_shared_mesh(g, "m").unwrap();
    assert!(
        revision(&scene, g) > uploaded,
        "B's buffers are not reused for A"
    );
    let mesh = scene.world.mesh(g).unwrap();
    assert_eq!(mesh.vertices[1].position, [1.0, 0.0, 0.0], "draws A");
}

#[test]
fn stop_after_a_play_replace_gives_the_next_user_a_newer_revision() {
    let mut scene = Scene::new();
    let (f, g) = (scene.add_entity("F".into()), scene.add_entity("G".into()));
    scene.define_mesh("m", triangle(1.0)).unwrap();
    let snapshot = SceneSnapshot::capture(&scene);
    scene.define_mesh("m", triangle(2.0)).unwrap();
    scene.set_shared_mesh(f, "m").unwrap();
    let uploaded = revision(&scene, f);
    snapshot.restore(&mut scene);
    scene.set_shared_mesh(g, "m").unwrap();
    assert!(
        revision(&scene, g) > uploaded,
        "Play's buffers are not reused"
    );
    let mesh = scene.world.mesh(g).unwrap();
    assert_eq!(
        mesh.vertices[1].position,
        [1.0, 0.0, 0.0],
        "draws the edit data"
    );
}

#[test]
fn an_unchanged_entry_put_back_keeps_its_revision() {
    let mut scene = Scene::new();
    scene.define_mesh("m", triangle(1.0)).unwrap();
    let before = scene.meshes["m"].revision;
    let library = scene.meshes.clone();
    super::put_back(library, &mut scene.meshes);
    assert_eq!(scene.meshes["m"].revision, before, "nothing to re-upload");
}
