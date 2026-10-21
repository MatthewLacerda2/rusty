//! Invariant 4 (#1171): degenerate input either builds or returns a [`Skip`] that
//! logs a warning naming the entity. It never panics, and never vanishes silently.

use std::cell::RefCell;

use glam::Vec3;

use super::{turns, world_poses, SCALES};
use crate::components::{CapsuleAxis, ColliderShape};
use crate::physics::shape::build::{build_shape_for, try_build_shape};
use crate::physics::shape::mesh::Skip;
use crate::scripting::ConsoleLogs;

/// Every primitive variant at zero size.
const FLAT: [ColliderShape; 4] = [
    ColliderShape::Box { size: Vec3::ZERO },
    ColliderShape::Sphere { radius: 0.0 },
    ColliderShape::Cylinder {
        radius: 0.0,
        height: 0.0,
    },
    ColliderShape::Capsule {
        radius: 0.0,
        height: 0.0,
        axis: CapsuleAxis::X,
    },
];

#[test]
fn a_zero_sized_primitive_still_builds() {
    let zero_scale = [Vec3::ZERO, Vec3::new(0.0, 1.0, 1.0)];
    for shape in &FLAT {
        for scale in SCALES.into_iter().chain(zero_scale) {
            assert!(
                try_build_shape(shape, scale, None).is_ok(),
                "{shape:?} {scale}"
            );
        }
    }
    for shape in super::all_samples().iter().filter(|s| s.mesh.is_none()) {
        assert!(try_build_shape(&shape.shape, Vec3::ZERO, None).is_ok());
    }
}

type Geometry = (Vec<[f32; 3]>, Vec<u32>);

/// A bad mesh: what it is, its geometry, and what a hull and a trimesh built
/// from it come to.
type BadMesh = (&'static str, Option<Geometry>, [Result<(), Skip>; 2]);

fn bad_meshes() -> Vec<BadMesh> {
    let tri = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let few = |have, need| Err(Skip::TooFewVertices { have, need });
    vec![
        (
            "one triangle",
            Some((tri.clone(), vec![0, 1, 2])),
            [few(3, 4), Ok(())],
        ),
        (
            "an index out of range",
            Some((vec![tri[0], tri[1], tri[2], [0.0, 0.0, 1.0]], vec![0, 1, 7])),
            // A hull reads no indices, so a stale one never drops it.
            [
                Ok(()),
                Err(Skip::IndexOutOfRange {
                    index: 7,
                    vertices: 4,
                }),
            ],
        ),
        (
            "an empty mesh",
            Some((vec![], vec![])),
            [few(0, 4), few(0, 3)],
        ),
        (
            "no mesh at all",
            None,
            [Err(Skip::NoMesh), Err(Skip::NoMesh)],
        ),
        (
            "a flat point",
            Some((vec![[0.0; 3]; 4], vec![0, 1, 2])),
            [Err(Skip::NoHull), Err(Skip::ZeroArea)],
        ),
    ]
}

#[test]
fn a_bad_mesh_builds_or_says_why() {
    for (what, geometry, want) in bad_meshes() {
        for (convex, want) in [true, false].into_iter().zip(want) {
            let shape = ColliderShape::Mesh {
                convex,
                local_min: Vec3::ZERO,
                local_max: Vec3::ONE,
            };
            let mesh = geometry.as_ref().map(|(p, i)| (p.as_slice(), i.as_slice()));
            for (scale, turn) in SCALES.into_iter().zip(turns().iter().cycle()) {
                let pose = world_poses(scale, turn.0)[1];
                let got = try_build_shape(&shape, pose.scale, mesh).map(|_| ());
                assert_eq!(got, want, "{what}, convex {convex}, scale {scale}");
            }
        }
    }
}

#[test]
fn a_skipped_mesh_logs_a_warning_naming_its_entity() {
    crate::logging::init();
    let console = RefCell::new(ConsoleLogs::new());
    crate::logging::forward(&console);
    for (n, (what, geometry, _)) in bad_meshes().into_iter().enumerate() {
        let id = 9100 + n as u32;
        let mesh = geometry.as_ref().map(|(p, i)| (p.as_slice(), i.as_slice()));
        let shape = ColliderShape::Mesh {
            convex: true,
            local_min: Vec3::ZERO,
            local_max: Vec3::ONE,
        };
        let built = build_shape_for(id, &shape, Vec3::ONE, mesh).is_some();
        let logged = crate::logging::forward(&console);
        let named = logged.iter().any(|m| m.contains(&format!("entity {id}:")));
        assert!(built != named, "{what}: built {built}, warned {logged:?}");
    }
}
