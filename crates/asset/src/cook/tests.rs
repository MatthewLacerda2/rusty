//! Cooking (#2332): the threaded steps give the single-threaded asset, bit for bit.

use super::fixture;
use super::par::with_threads;
use crate::{clusters, import_file};

#[test]
fn a_threaded_import_is_the_single_threaded_one_bit_for_bit() {
    let path = fixture::model("threads");
    let serial = with_threads(1, || import_file(&path).unwrap());
    assert_eq!(serial.sub_meshes.len(), 3, "the fixture's three objects");
    let grid = &serial.sub_meshes[0];
    assert!(grid.indices.len() / 3 >= clusters::MIN_TRIANGLES);
    assert!(grid.clusters.is_some(), "the grid is clustered");
    assert!(
        grid.vertices.iter().any(|v| v.lightmap_uv != [0.0; 2]),
        "the grid is unwrapped"
    );
    assert!(
        grid.vertices
            .iter()
            .any(|v| v.tangent != [1.0, 0.0, 0.0, 1.0]),
        "its tangents are generated"
    );
    for threads in [2, 3, 8] {
        let threaded = with_threads(threads, || import_file(&path).unwrap());
        // `PartialEq` compares floats by value; the printed form shows a -0 too.
        assert_eq!(threaded, serial, "{threads} threads");
        assert_eq!(format!("{threaded:?}"), format!("{serial:?}"));
    }
}

#[test]
fn a_threaded_import_is_still_one_parse() {
    let path = fixture::model("one_parse");
    let before = crate::import_cache::parses();
    with_threads(4, || crate::imported(&path).unwrap());
    with_threads(4, || crate::imported(&path).unwrap());
    assert_eq!(crate::import_cache::parses() - before, 1);
}
