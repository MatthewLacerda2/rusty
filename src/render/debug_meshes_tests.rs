//! The re-upload rule (#1540): which geometry slots a renderer refreshes, and from
//! which mesh. Pure, so it runs with no adapter.

use super::{stale_uploads, UploadCandidate};
use crate::components::MeshRevision;
use crate::render::MeshId;

fn floor(revision: MeshRevision, entity: u32) -> UploadCandidate {
    UploadCandidate {
        id: MeshId("Asset|floor".into()),
        revision,
        entity,
        has_geometry: true,
    }
}

/// The entity each slot uploads from, given what is resident.
fn picks(resident: Option<MeshRevision>, candidates: Vec<UploadCandidate>) -> Vec<u32> {
    let uploads = stale_uploads(candidates, |_: &MeshId| resident);
    uploads.values().map(|&(_, entity)| entity).collect()
}

#[test]
fn a_slot_uploads_only_when_a_mesh_is_newer_than_its_buffers() {
    let (old, new) = (MeshRevision::next(), MeshRevision::next());
    assert_eq!(picks(None, vec![floor(old, 1)]), [1], "not resident");
    assert_eq!(picks(Some(old), vec![floor(new, 1)]), [1], "re-imported");
    assert!(
        picks(Some(old), vec![floor(old, 1)]).is_empty(),
        "up to date"
    );
    assert!(
        picks(Some(new), vec![floor(old, 1)]).is_empty(),
        "older mesh"
    );
}

#[test]
fn the_newest_mesh_with_geometry_wins_a_slot() {
    let (old, new, newest) = (
        MeshRevision::next(),
        MeshRevision::next(),
        MeshRevision::next(),
    );
    let both = vec![floor(new, 2), floor(old, 1)];
    assert_eq!(picks(None, both), [2], "newest, whatever the order");
    let empty = UploadCandidate {
        has_geometry: false,
        ..floor(newest, 3)
    };
    assert_eq!(
        picks(None, vec![floor(new, 2), empty]),
        [2],
        "empty never wins"
    );
}
