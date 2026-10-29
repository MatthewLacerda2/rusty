//! Unit tests for [`MeshId`] geometry-identity keying (#127). Split out of `mod.rs`
//! to keep that file under the size cap.

use crate::render::MeshId;
use crate::scene::{DirtyFlag, MeshComponent};

fn mesh(primitive: &str, asset: Option<&str>) -> MeshComponent {
    MeshComponent {
        asset_ref: asset.map(String::from),
        is_dirty: DirtyFlag::new(false),
        ..MeshComponent::new(primitive.to_string(), Vec::new(), Vec::new())
    }
}

#[test]
fn identical_geometry_shares_one_id() {
    // The key is the source, not the entity: two box meshes dedup to one
    // buffer; two references to the same asset sub-object likewise.
    assert_eq!(
        MeshId::from_mesh(&mesh("Box", None)),
        MeshId::from_mesh(&mesh("Box", None))
    );
    assert_eq!(
        MeshId::from_mesh(&mesh("Asset", Some("models/crates.glb::Barrel"))),
        MeshId::from_mesh(&mesh("Asset", Some("models/crates.glb::Barrel"))),
    );
}

#[test]
fn distinct_geometry_gets_distinct_ids() {
    assert_ne!(
        MeshId::from_mesh(&mesh("Box", None)),
        MeshId::from_mesh(&mesh("Sphere", None))
    );
    assert_ne!(
        MeshId::from_mesh(&mesh("Asset", Some("models/crates.glb::Barrel"))),
        MeshId::from_mesh(&mesh("Asset", Some("models/crates.glb::Crate"))),
    );
}

#[test]
fn a_subdivided_plane_keys_apart_from_the_single_quad() {
    let quad = mesh("Plane", None);
    let grid = MeshComponent {
        subdivisions: 64,
        ..mesh("Plane", None)
    };
    assert_eq!(MeshId::from_mesh(&quad).0, "Plane|", "1 keeps today's key");
    assert_ne!(MeshId::from_mesh(&quad), MeshId::from_mesh(&grid));
}

/// The default scene's two waters, one Plane of each kind (#1524): each uploads its
/// own buffer with its own index count, whichever is visited first.
#[test]
fn gpu_planes_with_different_subdivisions_upload_two_meshes() {
    use crate::scene::authoring::{create_entity, rendering::mesh::set_subdivisions, Primitive};
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let mut scene = crate::scene::Scene::new();
    let quad = create_entity(&mut scene, "Quad", Some(Primitive::Plane));
    let grid = create_entity(&mut scene, "Grid", Some(Primitive::Plane));
    set_subdivisions(&mut scene.world.mesh_mut(grid).unwrap(), 8).unwrap();
    renderer.upload_scene_meshes(&scene);
    for (id, indices) in [(quad, 6), (grid, 8 * 8 * 6)] {
        let key = MeshId::from_mesh(&scene.world.mesh(id).unwrap());
        assert_eq!(renderer.gpu_meshes[&key].num_indices, indices, "{key:?}");
    }
}
