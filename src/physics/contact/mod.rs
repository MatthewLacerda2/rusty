//! src/physics/contact/mod.rs — the contact generation rapier runs, with the
//! fixes a rolling body needs (#2071).
//!
//! rapier 0.36 fed energy into a `Cylinder` rolling on its side: a barrel let go
//! at 0.3 m/s on a box floor was past 1.7 m/s a minute later. Three parts of
//! parry's contact generation did it, measured one at a time in raw rapier:
//!
//! 1. **Reused contact points.** parry keeps last tick's points while the
//!    shapes have barely moved (`try_update_contacts`). A rolling body's contact
//!    point is the one point of it that does not move, so the test passes every
//!    other tick while the centre runs on; the floor then pushes up at a point
//!    behind the centre, and that torque spins the roll up. A lying `Capsule`
//!    goes through the same reuse (0.30 → 0.42 m/s), so it is fixed here too.
//! 2. **A noisy normal.** On a box floor GJK's normal for a curved side wobbles
//!    by up to tens of degrees on some ticks; a tilted normal pushes the body
//!    along. A triangle-mesh floor snaps its normal to the face and has no wobble,
//!    so it is the box (and any non-mesh floor) that pays.
//! 3. **Warm-started impulses** carried from tick to tick: with points fresh
//!    every tick and the normal snapped to the box face, a cylinder on a box
//!    still sped up, and stopped doing so only with warm-starting off.
//!
//! So every pair with a rolling cylinder or capsule recomputes its points each
//! tick, and a rolling cylinder's side meets the world as a convex hull of
//! [`side::SIDES`] facets,
//! as a barrel's convex mesh collider does in Unity. A facet lies flat on the
//! floor, so its normal and its points are exact, and each facet the hull rolls
//! over takes a little energy: it rolls on and comes to rest. Its cap meets the
//! world as the true cylinder, since parry builds a hull face's contact from its
//! first four corners only (a known gap in `ConvexPolyhedron`), which would stand
//! a 64-gon cap on a sliver of its rim. Raycasts and every other query see the
//! true shapes.
//!
//! **Only what rolls.** A cylinder or capsule rolls when it is on a dynamic body,
//! and only then is it built to roll ([`rolling_cylinder`], [`never_recycled`],
//! called from `shape::build`). A static or kinematic cylinder (a pillar, a
//! moving platform) stays the exact `Cylinder`, so a body sliding round it
//! meets a smooth side. The dispatcher sees shapes, never bodies, so a rolling
//! cylinder is told apart by its shape: a `RoundCylinder` with no rounding,
//! which parry treats as the same cylinder everywhere (contacts, mass, rays),
//! and which the engine builds for nothing else.
//!
//! **Fresh means every tick.** Besides parry's reuse inside a manifold, rapier
//! recycles a whole pair (`IntegrationParameters::contact_recycling`) while its
//! relative pose drifts less than 5 cm, a window scaled by the pair's size: on a
//! crate-sized floor a slow roll would keep frozen contacts for ticks at a time
//! and gain energy as fault 1 does. rapier never recycles a pair with a user
//! hook in it, so a rolling collider asks for one ([`never_recycled`]); the
//! world steps with no hooks (`()`), so the hook does nothing.

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

type Manifold = ContactManifold<ContactManifoldData, ContactData>;

/// The narrow phase every `PhysicsWorld` steps with: [`Rolling`] ahead of parry's
/// default dispatcher, which answers everything `Rolling` leaves alone.
pub(super) fn narrow_phase() -> NarrowPhase {
    NarrowPhase::with_query_dispatcher(Rolling::default().chain(DefaultQueryDispatcher))
}

/// The collider of a cylinder on a dynamic body: the same cylinder, as a
/// `RoundCylinder` with no rounding, the mark [`Rolling`] gives a side hull.
pub(in crate::physics) fn rolling_cylinder(half_height: Real, radius: Real) -> ColliderBuilder {
    ColliderBuilder::round_cylinder(half_height, radius, 0.0)
}

/// `collider` with its pairs kept out of rapier's contact recycling, by asking
/// for a solver-contact hook the world never acts on (the module doc says why).
pub(in crate::physics) fn never_recycled(collider: ColliderBuilder) -> ColliderBuilder {
    collider.active_hooks(ActiveHooks::MODIFY_SOLVER_CONTACTS)
}

/// The contact generator for pairs with a rolling shape in them (the module doc
/// says why). It answers `Unsupported` for everything else, so the chained
/// default dispatcher handles it unchanged.
#[derive(Default)]
struct Rolling {
    hulls: side::Hulls,
}

/// Whether `shape` rolls on a curved side, and so gets fresh points each tick:
/// a rolling cylinder ([`side::rolling`]) or any capsule.
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

/// Every one-shot query goes to the default dispatcher, on the true shapes.
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
