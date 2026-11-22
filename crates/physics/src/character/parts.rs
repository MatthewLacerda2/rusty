use rapier3d::parry::bounding_volume::Aabb;
use rapier3d::parry::partitioning::{BvhLeafCost, BvhNode};
use rapier3d::parry::query::details::NormalConstraints;
use rapier3d::parry::query::{QueryDispatcher, Ray, RayCast, ShapeCastHit, ShapeCastOptions};
use rapier3d::parry::shape::Shape;
use rapier3d::prelude::*;

use super::composite::{AtPart, Constraints, Parts};
use super::cost::{self, Work};
use super::normals::exact_normal;

/// Under this time of impact parry decides a support-map cast by the contact normal
/// at the start pose: a pair moving along or away from it is no hit.
const NEAR_TOI: Real = 1.0e-4;

/// Not parry's `CompositeShapeRef::cast_shape`: it drops the parts' normal constraints.
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
        let cast = cast.until(best);
        let mut found = None;
        parts.with_part(id, pos12, vel12, &mut |at, part, constraints| {
            found = cast.part(&at, part, constraints).map(|h| match h {
                PartHit::Done(h) => PartHit::Done(at.back(h)),
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
            parts.with_part(id, pos12, vel12, &mut |at, part, constraints| {
                done = at.back(cast.finish(raw, &at, part, constraints));
            });
            done
        }
    };
    hit.subshape1 = id;
    Some(hit)
}

/// No normal drops a `Late` hit: only its time of impact counts until it wins.
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

    fn part(&self, at: &AtPart, part: &dyn Shape, c: Constraints) -> Option<PartHit> {
        let (g2, options) = (self.g2, self.options);
        let (local, vel) = (&at.pos12, at.vel12);
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

    fn finish(
        &self,
        raw: ShapeCastHit,
        at: &AtPart,
        part: &dyn Shape,
        c: Constraints,
    ) -> ShapeCastHit {
        let (local, vel) = (&at.pos12, at.vel12);
        let hit = exact_normal(
            self.dispatcher,
            raw,
            local,
            vel,
            [part, self.g2],
            self.options,
        );
        let kept = c.with(|c| constrained(hit, c, local, vel, self.options));
        debug_assert!(
            kept.is_some(),
            "a hit past NEAR_TOI was dropped by its normal"
        );
        kept.unwrap_or(hit)
    }
}

/// A hit within [`NEAR_TOI`] is judged again as parry would: by the corrected normal against
/// `vel`, so a capsule sliding along a seam's face goes on. Correcting only the normal
/// and keeping the hit stalled the walk.
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
    // Not at penetration (`ground_below`'s probe): parry keeps such a hit.
    let rejudged = !options.stop_at_penetration && hit.time_of_impact < NEAR_TOI;
    if rejudged && normal.dot(vel) >= 0.0 {
        return None;
    }
    Some(with_normal(hit, normal, pos12))
}

fn corrected(hit: &ShapeCastHit, constraints: Option<&dyn NormalConstraints>) -> Option<Vector> {
    let mut normal = hit.normal1;
    constraints?
        .project_local_normal_mut(&mut normal)
        .then_some(normal)
}

fn with_normal(mut hit: ShapeCastHit, normal: Vector, pos12: &Pose) -> ShapeCastHit {
    hit.normal1 = normal;
    hit.normal2 = pos12.rotation.inverse() * -normal;
    hit
}

#[cfg(test)]
#[path = "parts_tests.rs"]
mod tests;
