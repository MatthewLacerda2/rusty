//! An edit of one instance of a shared model (#2351), in place or by pointing it
//! at a library mesh (`Mesh.Create`'s path), copies or replaces that instance's
//! geometry alone: its sibling and the import cache keep theirs.

use super::sharing_tests::{cached, level, shares};
use rusty_asset::mesh_data::Shared;
use rusty_components::mesh::{copies, MeshData};

#[test]
fn an_edit_of_one_instance_leaves_its_sibling_and_the_cache_alone() {
    let (mut scene, ids, path) = level("rusty_sharing_edit", 2);
    let (a, b) = (ids[0], ids[1]);
    let before = scene.world.mesh(a).unwrap().revision;
    let from = copies();
    scene.world.mesh_mut(a).unwrap().vertices_mut()[0].position = [5.0; 3];
    assert_eq!(
        copies() - from,
        1,
        "the edited instance's vertices, copied once"
    );
    let buffers = cached(&path);
    assert_eq!(buffers.0[0].position, [0.0; 3], "the cache is untouched");
    assert!(
        shares(&scene, b, &buffers),
        "the sibling still shares the cache"
    );
    let edited = scene.world.mesh(a).unwrap();
    assert!(!Shared::same(&edited.vertices, &buffers.0));
    assert!(
        Shared::same(&edited.indices, &buffers.1),
        "its indices stay shared"
    );
    assert!(edited.revision > before, "re-uploaded");
}

#[test]
fn mesh_create_over_one_instance_leaves_its_sibling_and_the_cache_alone() {
    let (mut scene, ids, path) = level("rusty_sharing_create", 2);
    let (a, b) = (ids[0], ids[1]);
    let before = scene.world.mesh(a).unwrap().revision;
    let triangle = MeshData {
        positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
        indices: vec![0, 1, 2],
        ..Default::default()
    };
    scene.define_mesh("Tri", triangle).unwrap();
    scene.begin_edits(); // one Undo step, as the editor's frame opens one
    crate::mesh::set_shared_mesh(&mut scene, a, "Tri").unwrap();
    scene.commit_edits(None);
    let buffers = cached(&path);
    assert_eq!(scene.world.mesh(a).unwrap().indices.len(), 3);
    assert!(
        scene.world.mesh(a).unwrap().revision > before,
        "re-uploaded"
    );
    assert_eq!(buffers.1.len(), 6, "the cache's quad");
    assert!(
        shares(&scene, b, &buffers),
        "the sibling still shares the cache"
    );

    assert!(scene.undo().is_some());
    assert!(shares(&scene, a, &buffers), "Undo shares the cache's again");
    assert!(shares(&scene, b, &buffers));
}
