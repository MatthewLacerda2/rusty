//! Unit tests for [`MeshId`] geometry-identity keying (#127). Split out of `mod.rs`
//! to keep that file under the size cap.

use super::MeshId;
use crate::scene::{DirtyFlag, MeshComponent};

fn mesh(primitive: &str, asset: Option<&str>) -> MeshComponent {
    MeshComponent {
        primitive_type: primitive.to_string(),
        asset_ref: asset.map(String::from),
        vertices: Vec::new(),
        indices: Vec::new(),
        bind_palette: Vec::new(),
        skin: None,
        clips: Vec::new(),
        pose_palette: Vec::new(),
        is_dirty: DirtyFlag::new(false),
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
