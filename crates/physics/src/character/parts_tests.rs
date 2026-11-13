use rapier3d::parry::query::{DefaultQueryDispatcher, ShapeCastOptions};
use rapier3d::parry::shape::{Capsule, CompositeShape, TriMesh};
use rapier3d::prelude::*;

use super::cast_parts;
use crate::shape::mesh::TRIMESH_FLAGS;

const SKIN: Real = 0.02;

/// A flat 2 m × 1 m floor at y = 0 split at x = 0 into two quads (four
/// triangles), so x = 0 is a seam between coplanar triangles.
fn floor() -> TriMesh {
    let v = |x: f32, z: f32| Vector::new(x, 0.0, z);
    let vertices = vec![
        v(-1., 0.),
        v(0., 0.),
        v(1., 0.),
        v(-1., 1.),
        v(0., 1.),
        v(1., 1.),
    ];
    let indices = vec![[0, 3, 1], [1, 3, 4], [1, 4, 2], [2, 4, 5]];
    TriMesh::with_flags(vertices, indices, TRIMESH_FLAGS).unwrap()
}

/// A capsule resting within its skin of the floor and of the seam's edge
/// (its foot 5 cm before it), walking +x across it.
fn walk(mesh: &TriMesh, constrained: bool) -> Option<ShapeCastHit> {
    let capsule = Capsule::new_y(0.5, 0.3);
    let pose = Pose::translation(-0.05, 0.8 + SKIN * 0.5, 0.5);
    let options = ShapeCastOptions {
        target_distance: SKIN,
        stop_at_penetration: false,
        // Short of the far quad's diagonal (a hit ahead, whose normal the
        // other test checks).
        max_time_of_impact: 0.3,
        compute_impact_geometry_on_penetration: true,
    };
    let (vel, d) = (Vector::X, &DefaultQueryDispatcher);
    if constrained {
        cast_parts(
            d,
            mesh as &dyn CompositeShape,
            &pose,
            vel,
            &capsule,
            options,
        )
    } else {
        let parts = rapier3d::parry::shape::CompositeShapeRef(mesh);
        parts
            .cast_shape(d, &pose, vel, &capsule, options)
            .map(|(_, h)| h)
    }
}

#[test]
fn a_walk_along_a_flat_seam_goes_on() {
    let mesh = floor();
    // parry's own composite cast stops the capsule on the seam's edge.
    let stopped = walk(&mesh, false).expect("parry's cast meets the seam");
    assert!(stopped.time_of_impact < 1e-3, "{stopped:?}");
    let ours = walk(&mesh, true);
    assert!(ours.is_none(), "{ours:?}");
}

#[test]
fn a_seam_met_ahead_takes_the_face_normal() {
    let (mesh, capsule) = (floor(), Capsule::new_y(0.5, 0.3));
    // Over the near quad's diagonal: parry meets the far edge 0.23 m on,
    // its normal leaning back 14°.
    let pose = Pose::translation(-0.31, 0.8 + SKIN * 0.5, 0.5);
    let options = ShapeCastOptions {
        target_distance: SKIN,
        stop_at_penetration: false,
        max_time_of_impact: 0.3,
        compute_impact_geometry_on_penetration: true,
    };
    let (vel, d) = (Vector::X, &DefaultQueryDispatcher);
    let parts = rapier3d::parry::shape::CompositeShapeRef(&mesh);
    let (_, raw) = parts.cast_shape(d, &pose, vel, &capsule, options).unwrap();
    assert!(raw.normal1.y < 0.99, "{raw:?}");
    let hit = cast_parts(d, &mesh, &pose, vel, &capsule, options).expect("the seam");
    assert!(hit.normal1.y > 0.9999, "{hit:?}");
    assert!(hit.normal2.y < -0.9999, "{hit:?}");
}
