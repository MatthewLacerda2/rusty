//! crates/render/src/scene/library_gpu_tests.rs — the renderer and the scene's mesh
//! library (#2059).

use super::fixture::{frame, yard, RES};

/// `Mesh.DeleteAsset` (#2059) frees the deleted library mesh's buffers on the next
/// frame, and a mesh made again under its name uploads afresh.
#[test]
fn gpu_a_deleted_library_mesh_frees_its_buffers() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut scene = yard(1);
    let triangle = rusty_components::mesh::MeshData {
        positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        indices: vec![0, 1, 2],
        ..Default::default()
    };
    let id = scene.add_entity("Piece".into());
    scene.define_mesh("piece", triangle.clone()).unwrap();
    scene.set_shared_mesh(id, "piece").unwrap();
    let key = crate::MeshId("Scene|piece".into());
    frame(&mut r, &scene);
    assert!(r.gpu_meshes.contains_key(&key), "drawn, so uploaded");
    assert_eq!(scene.delete_mesh("piece", true).unwrap(), vec![id]);
    frame(&mut r, &scene);
    assert!(!r.gpu_meshes.contains_key(&key), "freed with the entry");
    scene.define_mesh("piece", triangle).unwrap();
    scene.set_shared_mesh(id, "piece").unwrap();
    frame(&mut r, &scene);
    assert!(
        r.gpu_meshes.contains_key(&key),
        "made again, uploaded again"
    );
}
