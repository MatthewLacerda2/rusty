//! The physics build (`try_build_shape`), the world AABB (`calculate_world_aabb`) and the
//! drawn primitive each read a `ColliderShape`; these run every variant through each.
mod bounds_tests;
mod degenerate_tests;
mod drawn_tests;
mod poses;
mod roster_tests;

use poses::{turns, world_poses, SCALES};

use glam::Vec3;

use super::fixture::off_centre_cylinder;
use rusty_components::{CapsuleAxis, ColliderShape};

pub(super) struct Sample {
    pub shape: ColliderShape,
    pub mesh: Option<(Vec<[f32; 3]>, Vec<u32>)>,
}

impl Sample {
    fn of(shape: ColliderShape) -> Self {
        Self { shape, mesh: None }
    }

    pub fn geometry(&self) -> Option<(&[[f32; 3]], &[u32])> {
        self.mesh
            .as_ref()
            .map(|(p, i)| (p.as_slice(), i.as_slice()))
    }
}

pub(super) const KINDS: [ColliderShape; 5] = [
    ColliderShape::Box { size: Vec3::ONE },
    ColliderShape::Sphere { radius: 1.0 },
    ColliderShape::Cylinder {
        radius: 1.0,
        height: 1.0,
    },
    ColliderShape::Capsule {
        radius: 1.0,
        height: 1.0,
        axis: CapsuleAxis::Y,
    },
    ColliderShape::Mesh {
        convex: false,
        local_min: Vec3::ZERO,
        local_max: Vec3::ZERO,
    },
];

/// No `_` arm: a new variant does not compile until it has samples here.
pub(super) fn samples(kind: &ColliderShape) -> Vec<Sample> {
    match kind {
        ColliderShape::Box { .. } => vec![Sample::of(ColliderShape::Box {
            size: Vec3::new(1.0, 2.0, 0.5),
        })],
        ColliderShape::Sphere { .. } => vec![Sample::of(ColliderShape::Sphere { radius: 0.5 })],
        ColliderShape::Cylinder { .. } => vec![Sample::of(ColliderShape::Cylinder {
            radius: 0.5,
            height: 2.0,
        })],
        ColliderShape::Capsule { .. } => {
            // Each axis, and one shorter than its caps (a sphere).
            let long = CapsuleAxis::ALL.map(|axis| (0.5, 3.0, axis));
            let short = [(0.5, 0.6, CapsuleAxis::Z)];
            (long.into_iter().chain(short))
                .map(|(radius, height, axis)| {
                    Sample::of(ColliderShape::Capsule {
                        radius,
                        height,
                        axis,
                    })
                })
                .collect()
        }
        ColliderShape::Mesh { .. } => [true, false].map(off_centre_mesh).into(),
    }
}

pub(super) fn all_samples() -> Vec<Sample> {
    KINDS.iter().flat_map(samples).collect()
}

fn off_centre_mesh(convex: bool) -> Sample {
    let (vertices, indices) = off_centre_cylinder();
    let points: Vec<Vec3> = vertices.iter().map(|v| Vec3::from(v.position)).collect();
    let (local_min, local_max) = points.iter().fold((Vec3::MAX, Vec3::MIN), |(lo, hi), &p| {
        (lo.min(p), hi.max(p))
    });
    Sample {
        shape: ColliderShape::Mesh {
            convex,
            local_min,
            local_max,
        },
        mesh: Some((points.iter().map(|p| p.to_array()).collect(), indices)),
    }
}
