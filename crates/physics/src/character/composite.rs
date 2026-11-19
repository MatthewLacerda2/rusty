//! crates/physics/src/character/composite.rs — the parts of a triangle mesh or
//! compound as the controller's part-by-part cast (`parts::cast_parts`) reads
//! them: a trimesh's triangles straight from the mesh, so a triangle's normal
//! constraints are built only when a hit asks for them, and the cast carried
//! into and out of each part's frame in one place.

use rapier3d::parry::query::details::NormalConstraints;
use rapier3d::parry::query::ShapeCastHit;
use rapier3d::parry::shape::{CompositeShape, Shape, TriMesh};
use rapier3d::prelude::*;

use super::cost::{self, Work};

/// A composite shape's parts, read straight from the trimesh when it is one,
/// so a triangle's normal constraints are built only when asked for
/// (parry's `map_part_at` builds them up front).
#[derive(Clone, Copy)]
pub(super) struct Parts<'a> {
    shape: &'a dyn CompositeShape,
    mesh: Option<&'a TriMesh>,
}

impl<'a> Parts<'a> {
    pub(super) fn of(g1: &'a dyn Shape) -> Option<Self> {
        let shape = g1.as_composite_shape()?;
        Some(Parts {
            shape,
            mesh: g1.as_trimesh(),
        })
    }

    pub(super) fn bvh(&self) -> &rapier3d::parry::partitioning::Bvh {
        self.shape.bvh()
    }

    /// Part `id` handed to `f`: the cast's `pos12` and `vel12` carried into
    /// the part's frame ([`AtPart`]), the part, and its normal constraints.
    pub(super) fn with_part(&self, id: u32, pos12: &Pose, vel12: Vector, f: &mut PartFn) {
        match self.mesh {
            Some(mesh) => f(
                AtPart::new(None, pos12, vel12),
                &mesh.triangle(id),
                Constraints::Mesh(mesh, id),
            ),
            None => self.shape.map_part_at(id, &mut |pose, part, c| {
                f(AtPart::new(pose, pos12, vel12), part, Constraints::Given(c))
            }),
        }
    }
}

/// What [`Parts::with_part`] hands a part to: the cast in its frame, its
/// shape and its constraints.
pub(super) type PartFn<'f> = dyn FnMut(AtPart, &dyn Shape, Constraints) + 'f;

/// A cast carried into one part's frame, and its hits carried back: the only
/// place that arithmetic is written, so the search and a late winner's
/// finish cannot drift apart.
#[derive(Clone, Copy)]
pub(super) struct AtPart<'p> {
    /// The part's pose in the composite's frame (`None`: the identity).
    pose: Option<&'p Pose>,
    /// The moving shape's pose in the part's frame.
    pub(super) pos12: Pose,
    /// The motion in the part's frame.
    pub(super) vel12: Vector,
}

impl<'p> AtPart<'p> {
    fn new(pose: Option<&'p Pose>, pos12: &Pose, vel12: Vector) -> Self {
        Self {
            pose,
            pos12: pose.map_or(*pos12, |p| p.inv_mul(pos12)),
            vel12: pose.map_or(vel12, |p| p.rotation.inverse() * vel12),
        }
    }

    /// `hit`, cast in the part's frame, in the composite's.
    pub(super) fn back(&self, hit: ShapeCastHit) -> ShapeCastHit {
        self.pose.map_or(hit, |p| hit.transform1_by(p))
    }
}

/// A part's normal constraints: a trimesh triangle's, built on demand, or
/// what parry handed over.
#[derive(Clone, Copy)]
pub(super) enum Constraints<'a> {
    Mesh(&'a TriMesh, u32),
    Given(Option<&'a dyn NormalConstraints>),
}

impl Constraints<'_> {
    pub(super) fn with<R>(self, f: impl FnOnce(Option<&dyn NormalConstraints>) -> R) -> R {
        match self {
            Constraints::Mesh(mesh, id) => {
                cost::count(Work::Constraints);
                let built = mesh.triangle_normal_constraints(id);
                f(built.as_ref().map(|c| c as &dyn NormalConstraints))
            }
            Constraints::Given(c) => f(c),
        }
    }
}
