//! src/physics/collision_events.rs — the per-tick solid-contact events
//! `PhysicsWorld::step` surfaces for `OnCollisionEnter/Stay/Exit` (#448).
//!
//! The twin of `trigger_events`: the current tick's touching pairs come from
//! rapier's contact graph (sensors never enter it — they live in the
//! intersection graph), and the enter/exit edges are recovered by diffing the
//! sorted pair keys against last tick's, exactly as the trigger edges are. Each
//! touching pair carries its **strongest** contact (Unity hands every point; one
//! is enough for impact sounds, damage and sticking): the manifold with the
//! largest solver impulse, and its deepest point. Only contacts that actually
//! touch count — rapier's speculative ones are ignored (`TOUCH_TOLERANCE`).

use crate::core::collections::Map;

use glam::Vec3;
use rapier3d::prelude::*;

use super::world::PhysicsWorld;

/// One contact as seen by the entity receiving the callback.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contact {
    /// World-space contact point.
    pub point: Vec3,
    /// Unit normal pointing out of the other collider, into the receiver — a
    /// ball resting on a floor reads `(0, 1, 0)`.
    pub normal: Vec3,
    /// The other side's velocity minus the receiver's, at the contact point,
    /// taken *before* this tick's solve — the closing speed of an impact.
    pub relative_velocity: Vec3,
    /// Total normal impulse the solver applied across the pair this tick.
    pub impulse: f32,
    /// The entity owning the other collider's rigid body (its compound root).
    pub other_body: u32,
}

/// One touching pair, keyed `(a, b)` with `a < b`, carrying `a`'s view of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollisionPair {
    pub a: u32,
    pub b: u32,
    /// The contact as `a` sees it (`other_body` is `b`'s body owner).
    pub contact: Contact,
    /// `a`'s own body owner — `b`'s `other_body`.
    pub body_a: u32,
}

impl CollisionPair {
    /// The sorted key the edge diff runs on.
    pub fn key(&self) -> (u32, u32) {
        (self.a, self.b)
    }

    /// The same pair keyed from `b`'s side.
    pub(super) fn swapped(&self) -> Self {
        let [_, (a, b, contact)] = self.sides();
        Self {
            a,
            b,
            contact,
            body_a: self.contact.other_body,
        }
    }

    /// Both receivers' views, A about B then B about A: `(id, other, contact)`.
    /// B's view flips the normal and the relative velocity.
    pub fn sides(&self) -> [(u32, u32, Contact); 2] {
        let c = self.contact;
        let flipped = Contact {
            normal: -c.normal,
            relative_velocity: -c.relative_velocity,
            other_body: self.body_a,
            ..c
        };
        [(self.a, self.b, c), (self.b, self.a, flipped)]
    }
}

/// Solid-contact events for one physics tick, every list sorted by pair key.
#[derive(Debug, Default)]
pub struct CollisionEvents {
    /// Pairs touching this tick that were not touching last tick.
    pub entered: Vec<CollisionPair>,
    /// Every pair touching this tick — the stay set, enter tick included (the
    /// same edge rule as `OnTrigger`).
    pub stayed: Vec<CollisionPair>,
    /// Pairs touching last tick that no longer touch. No contact data: there
    /// is no contact left to describe.
    pub exited: Vec<(u32, u32)>,
}

impl CollisionEvents {
    /// Diff last tick's sorted pair keys against this tick's sorted touching
    /// pairs. A pair *enters* only once the solver has pushed on it (impulse
    /// above zero): rapier finds a new contact after the tick's solve, so the
    /// impact lands a tick later, and entering then gives `OnCollisionEnter` the
    /// real impact impulse. It is also Unity's rule that a collision needs a
    /// dynamic body — two bodies the solver never pushes (kinematic against
    /// static) raise none. Once in, a pair stays while it touches.
    pub fn from_contact_sets(prev: &[(u32, u32)], current: Vec<CollisionPair>) -> Self {
        let was_in = |k: &(u32, u32)| prev.binary_search(k).is_ok();
        let current: Vec<CollisionPair> = current
            .into_iter()
            .filter(|p| p.contact.impulse > 0.0 || was_in(&p.key()))
            .collect();
        let keys: Vec<(u32, u32)> = current.iter().map(CollisionPair::key).collect();
        let entered = current
            .iter()
            .filter(|p| !was_in(&p.key()))
            .copied()
            .collect();
        let exited = prev
            .iter()
            .filter(|k| keys.binary_search(k).is_err())
            .copied()
            .collect();
        Self {
            entered,
            stayed: current,
            exited,
        }
    }

    /// The touching pair keys — carried to next tick's diff.
    pub fn stayed_keys(&self) -> Vec<(u32, u32)> {
        self.stayed.iter().map(CollisionPair::key).collect()
    }

    /// True when the tick produced no collision events at all.
    pub fn is_empty(&self) -> bool {
        self.entered.is_empty() && self.stayed.is_empty() && self.exited.is_empty()
    }
}

/// A body's velocity state before the solve: linear, angular, world centre of mass.
#[derive(Clone, Copy)]
pub(super) struct BodyVelocity {
    lin: Vec3,
    ang: Vec3,
    com: Vec3,
}

impl BodyVelocity {
    fn at(&self, point: Vec3) -> Vec3 {
        self.lin + self.ang.cross(point - self.com)
    }
}

/// Pre-solve velocities of every moving body, read by the contact collector.
pub(super) type VelocitySnapshot = Map<RigidBodyHandle, BodyVelocity>;

impl PhysicsWorld {
    /// Capture every non-fixed body's velocity before the pipeline solves the
    /// tick, so an impact reports its closing speed rather than the resolved one.
    pub(super) fn snapshot_velocities(&self) -> VelocitySnapshot {
        self.bodies
            .iter()
            .filter(|(_, b)| !b.is_fixed())
            .map(|(h, b)| {
                let v = BodyVelocity {
                    lin: b.linvel(),
                    ang: b.angvel(),
                    com: b.center_of_mass(),
                };
                (h, v)
            })
            .collect()
    }

    /// This tick's touching solid pairs (see [`TOUCH_TOLERANCE`]), sorted by
    /// key, each with its strongest contact. A disabled collider or body generates no contacts, so the pairs
    /// follow the same liveness rule as the queries (#521).
    pub(super) fn collect_contact_pairs(&self, pre: &VelocitySnapshot) -> Vec<CollisionPair> {
        let body_owner: Map<RigidBodyHandle, u32> =
            self.id_to_body.iter().map(|(&id, &h)| (h, id)).collect();
        let owner_of = |c: ColliderHandle| {
            let parent = self.colliders.get(c).and_then(|c| c.parent());
            parent.and_then(|h| body_owner.get(&h).copied())
        };
        let mut pairs = Vec::new();
        for pair in self.narrow_phase.contact_pairs() {
            let ids = (
                self.collider_to_id.get(&pair.collider1),
                self.collider_to_id.get(&pair.collider2),
            );
            let (Some(&id1), Some(&id2)) = ids else {
                continue;
            };
            let bodies = (owner_of(pair.collider1), owner_of(pair.collider2));
            let (Some(body1), Some(body2)) = bodies else {
                continue;
            };
            let Some((point, normal12)) = strongest_contact(pair, &self.bodies) else {
                continue;
            };
            let vel = |b| {
                let h = self.id_to_body.get(&b);
                h.and_then(|h| pre.get(h))
                    .map_or(Vec3::ZERO, |v| v.at(point))
            };
            // Seen by collider 1: the normal points out of 2 into 1, and the
            // relative velocity is 2's minus 1's.
            let seen_by_1 = Contact {
                point,
                normal: -normal12,
                relative_velocity: vel(body2) - vel(body1),
                impulse: pair.total_impulse_magnitude(),
                other_body: body2,
            };
            let pair = CollisionPair {
                a: id1,
                b: id2,
                contact: seen_by_1,
                body_a: body1,
            };
            pairs.push(if id1 < id2 { pair } else { pair.swapped() });
        }
        pairs.sort_unstable_by_key(CollisionPair::key);
        pairs
    }
}

/// How close two colliders' surfaces must be for their contact to count as
/// touching. rapier also reports *speculative* contacts — surfaces still apart
/// but within the prediction distance, whose impulse is zero — and a falling
/// body would otherwise "enter" a tick before it lands, with no impact impulse.
/// A resting body sits ~1 mm deep (rapier's allowed error), so 5 mm keeps a
/// resting contact steady while leaving speculative ones out.
const TOUCH_TOLERANCE: f32 = 0.005;

/// The pair's strongest touching contact: among the solver's manifolds with a
/// touching solver point, the one with the largest solver impulse (ties keep the
/// first), and its deepest point. Returns the world point (the midpoint of the two
/// surface points, rapier's effective contact point) and the manifold normal,
/// which points from collider 1 toward collider 2. `None` when nothing touches.
///
/// "Touching" is measured at the poses the step ended on: rapier stores a
/// contact's distance once per full narrow-phase update (at the start of a step)
/// and recycles it, so the two surface points are resolved through the bodies'
/// current poses and their gap along the normal is the live separation.
fn strongest_contact(pair: &ContactPair, bodies: &RigidBodySet) -> Option<(Vec3, Vec3)> {
    let deepest = |m: &ContactManifold| {
        m.data
            .solver_contacts
            .iter()
            .map(|c| {
                let (p1, p2) = m.data.solver_contact_world_points(c, bodies);
                ((p2 - p1).dot(m.data.normal), (p1 + p2) * 0.5)
            })
            .filter(|&(gap, _)| gap <= TOUCH_TOLERANCE)
            .min_by(|x, y| x.0.total_cmp(&y.0))
            .map(|(_, point)| point)
    };
    let (manifold, point) = pair
        .solver_manifolds()
        .iter()
        .filter_map(|m| Some((m, deepest(m)?)))
        .fold(
            None,
            |best: Option<(&ContactManifold, _)>, cand| match best {
                Some(b) if total_impulse(b.0) >= total_impulse(cand.0) => Some(b),
                _ => Some(cand),
            },
        )?;
    Some((point, manifold.data.normal))
}

/// Sum of a manifold's normal impulses (rapier's own helper trait is private).
fn total_impulse(m: &ContactManifold) -> f32 {
    m.points.iter().map(|p| p.data.impulse).sum()
}
