#[cfg(test)]
mod exact_tests;
#[cfg(test)]
mod fit_tests;
#[cfg(test)]
mod fixture;
#[cfg(test)]
mod roll_tests;
mod side;
#[cfg(test)]
mod stand_tests;

use rapier3d::parry::query::{
    details::NormalConstraints, ClosestPoints, Contact, ContactManifold, ContactManifoldsWorkspace,
    DefaultQueryDispatcher, NonlinearRigidMotion, PersistentQueryDispatcher, QueryDispatcher,
    ShapeCastHit, ShapeCastOptions, ShapeDistance, ShapeIntersection, Unsupported,
};
use rapier3d::parry::shape::{Shape, ShapeType};
use rapier3d::prelude::{
    ActiveHooks, ColliderBuilder, ContactData, ContactManifoldData, NarrowPhase, Pose, Real, Vector,
};
use rusty_components::ColliderShape;

type Manifold = ContactManifold<ContactManifoldData, ContactData>;

pub(super) fn narrow_phase() -> NarrowPhase {
    NarrowPhase::with_query_dispatcher(Rolling::default().chain(DefaultQueryDispatcher))
}

/// On a dynamic body a `RoundCylinder` with no rounding, [`Rolling`]'s mark for a side hull, which
/// nothing else builds; else the exact `Cylinder`.
pub(crate) fn cylinder(half_height: Real, radius: Real, dynamic: bool) -> ColliderBuilder {
    if dynamic {
        ColliderBuilder::round_cylinder(half_height, radius, 0.0)
    } else {
        ColliderBuilder::cylinder(half_height, radius)
    }
}

/// Recycled contacts fed energy into rolling bodies: a cylinder or capsule on a dynamic body asks
/// for a solver-contact hook the world never acts on, which keeps its pairs out of recycling.
pub(crate) fn rolling(
    collider: ColliderBuilder,
    shape: &ColliderShape,
    dynamic: bool,
) -> ColliderBuilder {
    let rolls = matches!(
        shape,
        ColliderShape::Cylinder { .. } | ColliderShape::Capsule { .. }
    );
    if dynamic && rolls {
        collider.active_hooks(ActiveHooks::MODIFY_SOLVER_CONTACTS)
    } else {
        collider
    }
}

/// Answers `Unsupported` for everything else, so the chained default dispatcher handles it.
#[derive(Default)]
struct Rolling {
    hulls: side::Hulls,
}

fn rolls(shape: &dyn Shape) -> bool {
    shape.shape_type() == ShapeType::Capsule || side::rolling(shape).is_some()
}

impl PersistentQueryDispatcher<ContactManifoldData, ContactData> for Rolling {
    fn contact_manifolds(
        &self,
        pos12: &Pose,
        g1: &dyn Shape,
        g2: &dyn Shape,
        prediction: Real,
        manifolds: &mut Vec<Manifold>,
        workspace: &mut Option<ContactManifoldsWorkspace>,
    ) -> Result<(), Unsupported> {
        if !rolls(g1) && !rolls(g2) {
            return Err(Unsupported);
        }
        // Decided from last tick's normals, before they are cleared.
        let hull1 = side::rolling(g1).filter(|_| side::on_side(manifolds, false));
        let hull2 = side::rolling(g2).filter(|_| side::on_side(manifolds, true));
        let (hull1, hull2) = (
            hull1.and_then(|c| self.hulls.of(c)),
            hull2.and_then(|c| self.hulls.of(c)),
        );
        let s1: &dyn Shape = hull1.as_deref().map_or(g1, |h| h);
        let s2: &dyn Shape = hull2.as_deref().map_or(g2, |h| h);
        // Fresh points every tick: a reused point lags a rolling body and spins it up.
        for manifold in manifolds.iter_mut() {
            manifold.mark_shapes_deformed();
        }
        DefaultQueryDispatcher.contact_manifolds(pos12, s1, s2, prediction, manifolds, workspace)
    }

    fn contact_manifold_convex_convex(
        &self,
        _: &Pose,
        _: &dyn Shape,
        _: &dyn Shape,
        _: Option<&dyn NormalConstraints>,
        _: Option<&dyn NormalConstraints>,
        _: Real,
        _: &mut Manifold,
    ) -> Result<(), Unsupported> {
        Err(Unsupported)
    }
}

impl QueryDispatcher for Rolling {
    fn intersection_test(
        &self,
        _: &Pose,
        _: &dyn Shape,
        _: &dyn Shape,
    ) -> Result<ShapeIntersection, Unsupported> {
        Err(Unsupported)
    }

    fn distance(
        &self,
        _: &Pose,
        _: &dyn Shape,
        _: &dyn Shape,
    ) -> Result<ShapeDistance, Unsupported> {
        Err(Unsupported)
    }

    fn contact(
        &self,
        _: &Pose,
        _: &dyn Shape,
        _: &dyn Shape,
        _: Real,
    ) -> Result<Option<Contact>, Unsupported> {
        Err(Unsupported)
    }

    fn closest_points(
        &self,
        _: &Pose,
        _: &dyn Shape,
        _: &dyn Shape,
        _: Real,
    ) -> Result<ClosestPoints, Unsupported> {
        Err(Unsupported)
    }

    fn cast_shapes(
        &self,
        _: &Pose,
        _: Vector,
        _: &dyn Shape,
        _: &dyn Shape,
        _: ShapeCastOptions,
    ) -> Result<Option<ShapeCastHit>, Unsupported> {
        Err(Unsupported)
    }

    fn cast_shapes_nonlinear(
        &self,
        _: &NonlinearRigidMotion,
        _: &dyn Shape,
        _: &NonlinearRigidMotion,
        _: &dyn Shape,
        _: Real,
        _: Real,
        _: bool,
    ) -> Result<Option<ShapeCastHit>, Unsupported> {
        Err(Unsupported)
    }
}
