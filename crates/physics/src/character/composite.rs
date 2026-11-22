use rapier3d::parry::query::details::NormalConstraints;
use rapier3d::parry::query::ShapeCastHit;
use rapier3d::parry::shape::{CompositeShape, Shape, TriMesh};
use rapier3d::prelude::*;

use super::cost::{self, Work};

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

pub(super) type PartFn<'f> = dyn FnMut(AtPart, &dyn Shape, Constraints) + 'f;

#[derive(Clone, Copy)]
pub(super) struct AtPart<'p> {
    /// `None`: the identity.
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

    pub(super) fn back(&self, hit: ShapeCastHit) -> ShapeCastHit {
        self.pose.map_or(hit, |p| hit.transform1_by(p))
    }
}

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
