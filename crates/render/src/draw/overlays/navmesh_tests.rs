use super::*;

/// A navmesh tile past the buffer limit is cut at a whole quad, never mid-triangle.
#[test]
fn a_navmesh_past_the_limit_keeps_whole_quads() {
    assert_eq!(vertices_that_fit(12, 1 << 20), 12);
    assert_eq!(vertices_that_fit(600, 12 * 100), 96);
    assert_eq!(vertices_that_fit(600, 12 * 5), 0);
    assert_eq!(vertices_that_fit(0, 0), 0);
}

fn revision(generation: u64) -> NavMeshRevision {
    let source = NavMeshSource {
        instance: 1,
        width: 2,
        height: 1,
        grid: [0; 3],
    };
    NavMeshRevision { source, generation }
}

fn quad(y: f32) -> Vec<Vec3> {
    vec![Vec3::new(0.0, y, 0.0); 6]
}

/// A patch uploads only the tiles it names and keeps the others; a reset drops all.
#[test]
fn gpu_a_patch_rewrites_only_its_tiles() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let tiles = vec![(0, quad(0.0)), (1, quad(1.0))];
    let reset = Some(3);
    let full = r.set_navmesh(NavMeshPatch {
        revision: revision(1),
        reset,
        tiles,
    });
    assert_eq!(full, 2 * 6 * VERTEX_BYTES);
    let tiles = vec![(1, Vec::new())];
    let patch = r.set_navmesh(NavMeshPatch {
        revision: revision(2),
        reset: None,
        tiles,
    });
    assert_eq!(patch, 0, "an emptied tile uploads nothing");
    let kept: Vec<bool> = r.navmesh.tiles.iter().map(Option::is_some).collect();
    assert_eq!(kept, [true, false, false], "tile 0 kept, tile 1 emptied");
    assert_eq!(r.navmesh_revision(), Some(revision(2)));
    let tiles = vec![(9, quad(0.0))];
    let past = r.set_navmesh(NavMeshPatch {
        revision: revision(3),
        reset: Some(1),
        tiles,
    });
    assert_eq!(past, 0, "a tile past the count is ignored");
    assert!(
        r.navmesh.tiles.iter().all(Option::is_none),
        "the reset dropped tile 0"
    );
}
