//! A mesh's geometry revision (#1540): what every renderer compares against the
//! one it uploaded, so no renderer can hide a change from another.

use super::{MeshComponent, MeshRevision};

#[test]
fn every_new_geometry_is_newer_than_all_before_it() {
    let first = MeshRevision::next();
    let mesh = MeshComponent::new("Box", Vec::new(), Vec::new());
    assert!(mesh.revision > first, "a new mesh");
    assert!(MeshRevision::default() > mesh.revision, "a loaded one");
    assert!(MeshRevision::NONE < first, "NONE is older than any");
}

#[test]
fn a_clone_holds_the_same_geometry() {
    let mesh = MeshComponent::new("Box", Vec::new(), Vec::new());
    let copy = mesh.clone();
    assert_eq!(
        copy.revision, mesh.revision,
        "a play snapshot needs no upload"
    );
}
