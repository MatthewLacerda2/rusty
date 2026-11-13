//! crates/physics/src/shape/merge_skip_tests.rs — the trimesh flags merge duplicate vertices
//! (#1880), yet a mesh that merges into nothing, or one with a NaN or infinite vertex,
//! is still a [`Skip`] decided before parry sees it, never a panic inside parry.

use glam::Vec3;

use super::mesh::{build_mesh_shape, MeshFit, Skip};

#[test]
fn vertices_that_all_merge_into_one_are_still_zero_area() {
    let point = [[1.0, 2.0, 3.0]; 6];
    let mesh = (point.as_slice(), [0u32, 1, 2, 3, 4, 5].as_slice());
    assert_eq!(
        build_mesh_shape(MeshFit::Exact, Vec3::ONE, mesh).err(),
        Some(Skip::ZeroArea)
    );
}

#[test]
fn a_nan_or_infinite_vertex_is_skipped_by_both_shapes() {
    let tetra = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, f32::NAN, 0.0],
        [0.0, 0.0, 1.0],
    ];
    let mesh = (tetra.as_slice(), [0u32, 1, 2, 0, 1, 3].as_slice());
    for convex in [false, true] {
        let skip = build_mesh_shape(MeshFit::of(convex, false), Vec3::ONE, mesh).err();
        assert_eq!(
            skip,
            Some(Skip::NonFinite { vertex: 2 }),
            "convex: {convex}"
        );
    }
    let huge = Vec3::new(f32::MAX, 1.0, 1.0);
    let flat = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let skip = build_mesh_shape(MeshFit::Exact, huge, (&flat, &[0, 1, 2])).err();
    assert_eq!(skip, Some(Skip::NonFinite { vertex: 1 }));
}
