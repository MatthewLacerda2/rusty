//! src/physics/shape/conformance/ — every collider shape under every awkward scale
//! (#1171). Three places read a [`ColliderShape`]: the physics build
//! ([`super::build::try_build_shape`]), the world AABB
//! ([`ColliderComponent::calculate_world_aabb`]) and the primitive mesh the user
//! sees. Four bugs (#1131, #1132, #1140, #1141) were one variant missing one scale
//! in one of them; these tests run every variant through every scale and turn, so
//! the next one fails here. One file per invariant.

mod bounds_tests;
mod degenerate_tests;
mod drawn_tests;
mod poses;
mod roster_tests;

use poses::{turns, world_poses, SCALES};

use glam::Vec3;

use crate::components::{CapsuleAxis, ColliderShape};
use crate::scene::authoring::{primitive_geometry, Primitive};

/// One shape to test, with the rest-pose geometry a mesh collider builds from.
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

/// One shape of each variant: what [`samples`] is called with. `roster_tests`
/// holds it to serde's own list of variants.
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

/// The samples of `kind`'s variant. No `_` arm: a new variant does not compile
/// until it has samples here.
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

/// Every sample of every variant.
pub(super) fn all_samples() -> Vec<Sample> {
    KINDS.iter().flat_map(samples).collect()
}

/// A mesh collider over the Cylinder primitive moved off its origin, so its
/// local box is not centred.
fn off_centre_mesh(convex: bool) -> Sample {
    let (vertices, indices) = primitive_geometry(Primitive::Cylinder).expect("a mesh");
    let offset = Vec3::new(0.3, 0.2, -0.1);
    let points: Vec<Vec3> = vertices
        .iter()
        .map(|v| Vec3::from(v.position) + offset)
        .collect();
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
