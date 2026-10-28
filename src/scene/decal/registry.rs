//! The scene's decals (#639): a spawn-ordered ring with per-bucket budgets (#1517)
//! that fade the oldest out instead of popping it, per-decal handles, and the
//! fixed-tick ageing.
//!
//! **Buckets.** Every decal counts against one bucket, keyed by [`Decal::bucket`]:
//! its owner entity (a bone is its own owner), or the world for an unowned one.
//! Each owner keeps [`DecalLimits::max_per_owner`] whole decals, the world
//! [`DecalLimits::max_world`], and the scene [`DecalLimits::max_total`] across all
//! of them. Eviction is decided in one place, [`DecalSet::enforce`]: past a budget
//! the oldest whole decal *of that bucket* starts the [`EVICTION_FADE`], and past
//! the total the oldest whole one in the scene does. The ring itself stays in global
//! spawn order whatever the buckets, so the blend order never changes.

use std::collections::{BTreeMap, VecDeque};
use std::ops::Deref;

use glam::Vec3;

use super::{Decal, DecalOwner, DecalSpec};
use crate::core::settings::decals::DecalLimits;
use crate::scene::Scene;

/// Seconds an evicted decal takes to fade out, so the cap never pops a mark in view.
pub const EVICTION_FADE: f32 = 0.5;

/// Decals that may be fading out early at once (evicted or removed with a fade).
/// Only past that (more than `EVICTION_HEADROOM / EVICTION_FADE` = 128 evictions a
/// second, sustained) is the oldest fading one dropped outright, so the frame holds
/// at most `max_total + EVICTION_HEADROOM` decals.
pub const EVICTION_HEADROOM: usize = 64;

/// The scene's live decals, oldest first (the order they blend in), and the budgets
/// they are held to. Reads go through [`Deref`] to the ring; every write is a
/// method, so the budgets hold.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DecalSet {
    ring: VecDeque<Decal>,
    /// The next handle; never reset, so a stale id can never name a newer decal.
    next_id: u32,
    limits: DecalLimits,
}

impl Deref for DecalSet {
    type Target = VecDeque<Decal>;
    fn deref(&self) -> &Self::Target {
        &self.ring
    }
}

impl DecalSet {
    /// The budgets the decals are held to.
    pub fn limits(&self) -> DecalLimits {
        self.limits
    }

    /// Hold the decals to `limits` from now on; a lowered budget fades the oldest
    /// excess of each bucket out at once.
    pub fn set_limits(&mut self, limits: DecalLimits) {
        self.limits = limits;
        self.enforce();
    }

    /// Add `decal` under a fresh id, making room per the budgets. Returns its id.
    fn push(&mut self, mut decal: Decal) -> u32 {
        self.next_id += 1;
        decal.id = self.next_id;
        self.ring.push_back(decal);
        self.enforce();
        self.next_id
    }

    /// Fade out the oldest whole decals of every bucket over its budget, then the
    /// oldest in the scene while the total is over, then drop the oldest fading
    /// ones past [`EVICTION_HEADROOM`]. The one place eviction is decided.
    fn enforce(&mut self) {
        let mut whole = BTreeMap::<Option<u32>, u32>::new();
        for d in self.ring.iter().filter(|d| !d.retiring) {
            *whole.entry(d.bucket()).or_default() += 1;
        }
        let mut total: u32 = whole.values().sum();
        let limits = self.limits;
        let over = |bucket: Option<u32>, n: u32| {
            n > bucket.map_or(limits.max_world, |_| limits.max_per_owner)
        };
        if whole.iter().any(|(&b, &n)| over(b, n)) {
            for d in self.ring.iter_mut().filter(|d| !d.retiring) {
                let n = whole.get_mut(&d.bucket()).expect("counted above");
                if over(d.bucket(), *n) {
                    d.retire(EVICTION_FADE);
                    *n -= 1;
                    total -= 1;
                }
            }
        }
        for d in self.ring.iter_mut().filter(|d| !d.retiring) {
            if total <= limits.max_total {
                break;
            }
            d.retire(EVICTION_FADE);
            total -= 1;
        }
        let mut fading = self.ring.iter().filter(|d| d.retiring).count();
        while fading > EVICTION_HEADROOM {
            let at = self.ring.iter().position(|d| d.retiring);
            self.ring.remove(at.expect("counted above"));
            fading -= 1;
        }
    }

    /// Remove decal `id` now, or fade it out over `fade` seconds when `fade > 0`.
    /// `false` when no live decal has that id.
    pub fn remove(&mut self, id: u32, fade: f32) -> bool {
        let Some(at) = self.ring.iter().position(|d| d.id == id) else {
            return false;
        };
        if fade > 0.0 {
            self.ring[at].retire(fade);
            self.enforce();
        } else {
            self.ring.remove(at);
        }
        true
    }

    /// Drop every decal (a level reset, a scene load). Ids keep counting, and the
    /// budgets stay.
    pub fn clear(&mut self) {
        self.ring.clear();
    }

    /// Age every decal by `dt` sim seconds, then drop the ones whose lifetime ran
    /// out and the ones whose owner no longer exists (`exists`).
    pub fn tick(&mut self, dt: f32, exists: impl Fn(u32) -> bool) {
        for decal in &mut self.ring {
            decal.age += dt;
        }
        self.ring
            .retain(|d| !d.expired() && d.owner.is_none_or(|o| exists(o.entity)));
    }
}

impl Scene {
    /// Spawn a box-projector decal at a surface hit (the point + outward normal
    /// already produced by `Physics.Raycast`), dressed by `spec`, and return its id.
    /// With `spec.owner` set to a live entity, the box is kept in that entity's
    /// space from now on (a dead owner stamps nothing and returns `None`).
    pub fn spawn_decal(&mut self, point: Vec3, normal: Vec3, spec: DecalSpec) -> Option<u32> {
        let owner = spec.owner;
        let mut decal = Decal::from_hit(point, normal, spec);
        if let Some(entity) = owner {
            if !self.world.contains(entity) {
                return None;
            }
            // The world store resolves a moved subtree on read, so a script's
            // move of the owner earlier this tick is already in this matrix.
            let world = self.world.local_to_world(entity);
            // A zero-scale owner has no space to keep the box in: stamp it unowned.
            if world.determinant().abs() > f32::EPSILON {
                let local = world.inverse() * decal.model_matrix();
                decal.owner = Some(DecalOwner { entity, local });
            }
        }
        Some(self.decals.push(decal))
    }

    /// Drop every live decal (e.g. on level reset).
    pub fn clear_decals(&mut self) {
        self.decals.clear();
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "budget_tests.rs"]
mod budget_tests;
