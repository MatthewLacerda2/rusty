use rusty_core::collections::Map;

use glam::Vec3;
use rapier3d::prelude::*;

use crate::world::PhysicsWorld;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contact {
    pub point: Vec3,
    /// Unit normal out of the other collider, into the receiver.
    pub normal: Vec3,
    pub relative_velocity: Vec3,
    pub impulse: f32,
    pub other_body: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollisionPair {
    pub a: u32,
    pub b: u32,
    pub contact: Contact,
    pub body_a: u32,
}

impl CollisionPair {
    pub fn key(&self) -> (u32, u32) {
        (self.a, self.b)
    }

    pub(crate) fn swapped(&self) -> Self {
        let [_, (a, b, contact)] = self.sides();
        Self {
            a,
            b,
            contact,
            body_a: self.contact.other_body,
        }
    }

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

#[derive(Debug, Default)]
pub struct CollisionEvents {
    pub entered: Vec<CollisionPair>,
    pub stayed: Vec<CollisionPair>,
    pub exited: Vec<(u32, u32)>,
}

impl CollisionEvents {
    /// A pair enters only once the solver has pushed on it (impulse above zero): rapier finds a new
    /// contact after the tick's solve, so `OnCollisionEnter` gets the real impulse a tick later.
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

    pub fn stayed_keys(&self) -> Vec<(u32, u32)> {
        self.stayed.iter().map(CollisionPair::key).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.entered.is_empty() && self.stayed.is_empty() && self.exited.is_empty()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct BodyVelocity {
    pub(crate) lin: Vec3,
    pub(crate) ang: Vec3,
    com: Vec3,
}

impl BodyVelocity {
    fn at(&self, point: Vec3) -> Vec3 {
        self.lin + self.ang.cross(point - self.com)
    }
}

pub(crate) type VelocitySnapshot = Map<RigidBodyHandle, BodyVelocity>;

impl PhysicsWorld {
    pub(crate) fn snapshot_velocities(&self) -> VelocitySnapshot {
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

    pub(crate) fn collect_contact_pairs(&self, pre: &VelocitySnapshot) -> Vec<CollisionPair> {
        let owner_of = |c: ColliderHandle| {
            let parent = self.colliders.get(c).and_then(|c| c.parent());
            parent.and_then(|h| self.body_to_id.get(&h).copied())
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

/// rapier also reports speculative contacts (surfaces apart, zero impulse), which would make a
/// falling body enter a tick early. A resting body sits ~1 mm deep, so 5 mm keeps resting
/// contact steady.
const TOUCH_TOLERANCE: f32 = 0.005;
/// Touching is measured at the poses the step ended on: rapier stores contact distance once per
/// narrow-phase update and recycles it.
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

fn total_impulse(m: &ContactManifold) -> f32 {
    m.points.iter().map(|p| p.data.impulse).sum()
}
