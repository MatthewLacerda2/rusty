//! crates/physics/src/character/parts.rs — the controller's shape cast against a
//! triangle mesh or compound, part by part, with each part's normal
//! constraints applied inside the cast (#2075). parry's own composite cast
//! (`CompositeShapeRef::cast_shape`) drops the constraints its parts carry, so
//! a mesh's `FIX_INTERNAL_EDGES` (#1880) never reached a sweep: at a seam
//! between two coplanar triangles the capsule, resting within its skin of the
//! floor, met the next triangle's edge at distance 0, its closest-points normal
//! leaning back toward the capsule, and parry counted that as moving into it.
//! The walk lost part of a stride, and a fast one hopped off the floor.
//!
//! The cast does only the work that can change its answer (#2335): a
//! triangle's normal constraints are built only for a hit, and a hit's exact
//! normal (`normals::exact_normal`, a GJK `contact`) is taken at once only
//! where it can drop the hit, under [`NEAR_TOI`]. Further on, only the time of
//! impact decides which part wins, so the exact normal and the constraints are
//! taken for the winner alone, from the same pose and inputs: the same hit,
//! bit for bit.

use rapier3d::parry::bounding_volume::Aabb;
use rapier3d::parry::partitioning::{BvhLeafCost, BvhNode};
use rapier3d::parry::query::details::NormalConstraints;
use rapier3d::parry::query::{QueryDispatcher, Ray, RayCast, ShapeCastHit, ShapeCastOptions};
use rapier3d::parry::shape::{CompositeShape, Shape, TriMesh};
use rapier3d::prelude::*;

use super::cost::{self, Work};
use super::normals::exact_normal;

/// Under this time of impact parry decides a support-map cast by the contact
/// normal at the start pose (`cast_shapes_support_map_support_map`'s fallback,
/// taken with `compute_impact_geometry_on_penetration`, which rapier's
/// controller always sets): a pair moving along or away from it is no hit.
const NEAR_TOI: Real = 1.0e-4;

/// The earliest hit of `g2`, posed at `pos12` in `g1`'s frame and moving by
/// `vel12`, against any part of the triangle mesh or compound `g1` (`None`
/// for any other shape): parry's composite traversal, each part cast through
/// `dispatcher`, its normal made exact ([`exact_normal`]) and held to the
/// part's normal constraints ([`constrained`]). The hit's `subshape1` is the
/// struck part, as parry's.
pub(super) fn cast_parts(
    dispatcher: &dyn QueryDispatcher,
    g1: &dyn Shape,
    pos12: &Pose,
    vel12: Vector,
    g2: &dyn Shape,
    options: ShapeCastOptions,
) -> Option<ShapeCastHit> {
    let parts = Parts::of(g1)?;
    cost::count(Work::MeshCast);
    let cast = Cast {
        dispatcher,
        g2,
        options,
    };
    let aabb2 = g2.compute_aabb(pos12);
    let ray = Ray::new(Vector::ZERO, vel12);
    let shift = -aabb2.center();
    let margin = aabb2.half_extents() + Vector::splat(options.target_distance);
    let node_toi = |node: &BvhNode, best: Real| {
        let msum = Aabb {
            mins: node.mins() + shift - margin,
            maxs: node.maxs() + shift + margin,
        };
        msum.cast_local_ray(&ray, best, true).unwrap_or(Real::MAX)
    };
    let part_hit = |id: u32, best: Real| {
        cost::count(Work::Part);
        // A part can win only by a hit before the best so far.
        let cast = cast.until(best);
        let mut found = None;
        parts.with_part(id, &mut |pose, part, constraints| {
            let local = pose.map_or(*pos12, |p| p.inv_mul(pos12));
            let vel = pose.map_or(vel12, |p| p.rotation.inverse() * vel12);
            found = cast.part(&local, vel, part, constraints).map(|h| match h {
                PartHit::Done(h) => PartHit::Done(pose.map_or(h, |p| h.transform1_by(p))),
                late => late,
            });
        });
        found
    };
    let (id, best) = parts
        .bvh()
        .find_best(options.max_time_of_impact, node_toi, part_hit)?;
    let mut hit = match best {
        PartHit::Done(hit) => hit,
        PartHit::Late(raw) => {
            let mut done = raw;
            parts.with_part(id, &mut |pose, part, constraints| {
                let local = pose.map_or(*pos12, |p| p.inv_mul(pos12));
                let vel = pose.map_or(vel12, |p| p.rotation.inverse() * vel12);
                let h = cast.finish(raw, &local, vel, part, constraints);
                done = pose.map_or(h, |p| h.transform1_by(p));
            });
            done
        }
    };
    hit.subshape1 = id;
    Some(hit)
}

/// A composite shape's parts, read straight from the trimesh when it is one,
/// so a triangle's normal constraints are built only when asked for
/// (parry's `map_part_at` builds them up front).
#[derive(Clone, Copy)]
struct Parts<'a> {
    shape: &'a dyn CompositeShape,
    mesh: Option<&'a TriMesh>,
}

impl<'a> Parts<'a> {
    fn of(g1: &'a dyn Shape) -> Option<Self> {
        let shape = g1.as_composite_shape()?;
        Some(Parts {
            shape,
            mesh: g1.as_trimesh(),
        })
    }

    fn bvh(&self) -> &rapier3d::parry::partitioning::Bvh {
        self.shape.bvh()
    }

    /// Part `id`, its pose in the composite's frame (`None`: the identity)
    /// and its normal constraints, handed to `f`.
    fn with_part(&self, id: u32, f: &mut PartFn) {
        match self.mesh {
            Some(mesh) => f(None, &mesh.triangle(id), Constraints::Mesh(mesh, id)),
            None => self.shape.map_part_at(id, &mut |pose, part, c| {
                f(pose, part, Constraints::Given(c))
            }),
        }
    }
}

/// What [`Parts::with_part`] hands a part to: its pose, shape and constraints.
type PartFn<'f> = dyn FnMut(Option<&Pose>, &dyn Shape, Constraints) + 'f;

/// A part's normal constraints: a trimesh triangle's, built on demand, or
/// what parry handed over.
#[derive(Clone, Copy)]
enum Constraints<'a> {
    Mesh(&'a TriMesh, u32),
    Given(Option<&'a dyn NormalConstraints>),
}

impl Constraints<'_> {
    fn with<R>(self, f: impl FnOnce(Option<&dyn NormalConstraints>) -> R) -> R {
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

/// One part's hit as the search holds it: finished (its normal exact and
/// constrained, in the composite's frame), or `Late`: a support-map hit at
/// or past [`NEAR_TOI`], as cast in the part's own frame. No normal can drop a
/// late hit, so only its time of impact counts until it wins.
#[derive(Clone, Copy)]
enum PartHit {
    Done(ShapeCastHit),
    Late(ShapeCastHit),
}

impl BvhLeafCost for PartHit {
    fn cost(&self) -> Real {
        match self {
            PartHit::Done(h) | PartHit::Late(h) => h.time_of_impact,
        }
    }
}

/// What every part of one cast is cast with.
#[derive(Clone, Copy)]
struct Cast<'a> {
    dispatcher: &'a dyn QueryDispatcher,
    g2: &'a dyn Shape,
    options: ShapeCastOptions,
}

impl Cast<'_> {
    fn until(self, best: Real) -> Self {
        let options = ShapeCastOptions {
            max_time_of_impact: best,
            ..self.options
        };
        Self { options, ..self }
    }

    /// `part`'s hit, if any: a composite part cast part by part, a convex
    /// pair's hit left [`PartHit::Late`] where no normal can drop it.
    fn part(&self, local: &Pose, vel: Vector, part: &dyn Shape, c: Constraints) -> Option<PartHit> {
        let (g2, options) = (self.g2, self.options);
        let exact = part.as_support_map().is_some() && g2.as_support_map().is_some();
        let hit = if part.as_composite_shape().is_some() {
            cast_parts(self.dispatcher, part, local, vel, g2, options)?
        } else {
            let hit = self.dispatcher.cast_shapes(local, vel, part, g2, options);
            let hit = hit.ok().flatten()?;
            match exact {
                true if hit.time_of_impact >= NEAR_TOI => return Some(PartHit::Late(hit)),
                true => exact_normal(self.dispatcher, hit, local, vel, [part, g2], options),
                false => hit,
            }
        };
        c.with(|c| constrained(hit, c, local, vel, options))
            .map(PartHit::Done)
    }

    /// A late hit that won, finished as [`Self::part`] would have at once:
    /// its exact normal, then its constraints (which, past [`NEAR_TOI`],
    /// never drop it).
    fn finish(
        &self,
        raw: ShapeCastHit,
        local: &Pose,
        vel: Vector,
        part: &dyn Shape,
        c: Constraints,
    ) -> ShapeCastHit {
        let hit = exact_normal(
            self.dispatcher,
            raw,
            local,
            vel,
            [part, self.g2],
            self.options,
        );
        c.with(|c| match corrected(&hit, c) {
            Some(normal) => with_normal(hit, normal, local),
            None => hit,
        })
    }
}

/// A part's `hit` with its normal corrected by the part's `constraints` (none,
/// or a correction they refuse, leave it as cast), or `None` when it no longer
/// stands: a hit within [`NEAR_TOI`] is judged again the way parry judged it,
/// by the corrected normal against `vel`, so a capsule moving along a seam's
/// flat face goes on instead of stopping on the seam's edge. A cast that
/// stops at penetration (`ground_below`'s probe) keeps it, as parry does. Correcting only
/// the reported normal and keeping the hit met the same edge on the next slide
/// and stalled the walk for good (#2073).
fn constrained(
    hit: ShapeCastHit,
    constraints: Option<&dyn NormalConstraints>,
    pos12: &Pose,
    vel: Vector,
    options: ShapeCastOptions,
) -> Option<ShapeCastHit> {
    let Some(normal) = corrected(&hit, constraints) else {
        return Some(hit);
    };
    let rejudged = !options.stop_at_penetration && hit.time_of_impact < NEAR_TOI;
    if rejudged && normal.dot(vel) >= 0.0 {
        return None;
    }
    Some(with_normal(hit, normal, pos12))
}

/// `hit`'s normal as `constraints` correct it, or `None` when there are none
/// or they refuse the correction.
fn corrected(hit: &ShapeCastHit, constraints: Option<&dyn NormalConstraints>) -> Option<Vector> {
    let mut normal = hit.normal1;
    constraints?
        .project_local_normal_mut(&mut normal)
        .then_some(normal)
}

/// `hit` with its normal set to `normal` (in the part's frame, `pos12` the
/// capsule's pose there).
fn with_normal(mut hit: ShapeCastHit, normal: Vector, pos12: &Pose) -> ShapeCastHit {
    hit.normal1 = normal;
    hit.normal2 = pos12.rotation.inverse() * -normal;
    hit
}

#[cfg(test)]
#[path = "parts_tests.rs"]
mod tests;
