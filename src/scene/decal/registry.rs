//! The scene's decals (#639): a spawn-ordered ring with per-bucket budgets (#1517)
//! that fade the oldest out instead of popping it, per-decal handles, and the
//! fixed-tick ageing.
//!
//! **Buckets.** Every decal counts against one bucket, keyed by [`Decal::bucket`]:
//! its owner entity (a bone is its own owner), or the world for an unowned one.
//! Each owner keeps [`DecalLimits::max_per_owner`] whole decals, the world
//! [`DecalLimits::max_world`], and the scene [`DecalLimits::max_total`] across all
//! of them. Running counts (`counts::Counts`) follow every add, fade and drop, so
//! a spawn checks only its own bucket and the total. Eviction is decided in
//! [`DecalSet::push`] (a spawn) and [`DecalSet::set_limits`] (a lowered budget):
//! past a budget the oldest whole decal *of that bucket* starts the
//! [`EVICTION_FADE`], and past the total the oldest whole one in the scene does.
//! The ring itself stays in global spawn order whatever the buckets, so the blend
//! order never changes.

use std::collections::VecDeque;
use std::ops::Deref;

use glam::Vec3;

use super::counts::Counts;
use super::{Decal, DecalOwner, DecalSpec};
use crate::core::settings::decals::DecalLimits;
use crate::scene::Scene;

/// Seconds an evicted decal takes to fade out, so the cap never pops a mark in view.
pub const EVICTION_FADE: f32 = 0.5;

/// Decals a stream of spawns may have fading out at once ([`Decal::evicted`]).
/// Only past that (more than `EVICTION_HEADROOM / EVICTION_FADE` = 128 evictions a
/// second, sustained) is the oldest one a spawn evicted dropped outright. A fade a
/// script asks for and one a lowered limit starts are not counted: those run their
/// course, since nothing repeats them every frame.
pub const EVICTION_HEADROOM: usize = 64;

/// The scene's live decals, oldest first (the order they blend in), and the budgets
/// they are held to. Reads go through [`Deref`] to the ring; every write is a
/// method, so the budgets and the counts hold.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DecalSet {
    ring: VecDeque<Decal>,
    /// The next handle; never reset, so a stale id can never name a newer decal.
    next_id: u32,
    limits: DecalLimits,
    counts: Counts,
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

    /// The whole decals `bucket` may keep.
    fn budget(&self, bucket: Option<u32>) -> u32 {
        bucket.map_or(self.limits.max_world, |_| self.limits.max_per_owner)
    }

    /// Hold the decals to `limits` from now on; a lowered budget fades the oldest
    /// excess of each bucket out at once, then the oldest in the scene past the
    /// total. Every decal it touches fades: none is dropped.
    pub fn set_limits(&mut self, limits: DecalLimits) {
        self.limits = limits;
        for at in 0..self.ring.len() {
            let d = &self.ring[at];
            if !d.retiring && self.counts.whole(d.bucket()) > self.budget(d.bucket()) {
                self.fade_at(at, EVICTION_FADE, false);
            }
        }
        for at in 0..self.ring.len() {
            if self.counts.total <= limits.max_total {
                break;
            }
            if !self.ring[at].retiring {
                self.fade_at(at, EVICTION_FADE, false);
            }
        }
    }

    /// Add `decal` under a fresh id, making room per the budgets. Returns its id.
    fn push(&mut self, mut decal: Decal) -> u32 {
        self.next_id += 1;
        decal.id = self.next_id;
        self.insert(decal);
        self.next_id
    }

    /// Append `decal` as it stands (id, age, fade), newest in the blend order, making
    /// room per the budgets: only its bucket and the total can be over. A spawn and
    /// a load's carried decals (#1518) both come in here.
    pub(super) fn insert(&mut self, decal: Decal) {
        let bucket = decal.bucket();
        if decal.retiring {
            self.counts.evicting += usize::from(decal.evicted);
        } else {
            self.counts.add(bucket);
        }
        self.ring.push_back(decal);
        while self.counts.whole(bucket) > self.budget(bucket)
            && self.evict_oldest(|d| d.bucket() == bucket)
        {}
        while self.counts.total > self.limits.max_total && self.evict_oldest(|_| true) {}
        while self.counts.evicting > EVICTION_HEADROOM {
            let at = self.ring.iter().position(|d| d.evicted);
            let gone = self.ring.remove(at.expect("counted")).expect("in range");
            self.counts.forget(&gone);
        }
    }

    /// Start the oldest whole decal `pick` accepts on the [`EVICTION_FADE`], as a
    /// spawn's eviction. `false` when there is none.
    fn evict_oldest(&mut self, pick: impl Fn(&Decal) -> bool) -> bool {
        let at = self.ring.iter().position(|d| !d.retiring && pick(d));
        at.inspect(|&at| self.fade_at(at, EVICTION_FADE, true))
            .is_some()
    }

    /// Fade the decal at `at` out over `seconds`; the first fade takes it off its
    /// budgets, and `evicted` marks a spawn's eviction ([`Decal::evicted`]).
    fn fade_at(&mut self, at: usize, seconds: f32, evicted: bool) {
        let d = &mut self.ring[at];
        if !d.retiring {
            self.counts.sub(d.bucket());
            d.evicted = evicted;
            self.counts.evicting += usize::from(evicted);
        }
        d.retire(seconds);
    }

    /// Remove decal `id` now, or fade it out over `fade` seconds when `fade > 0`.
    /// `false` when no live decal has that id.
    pub fn remove(&mut self, id: u32, fade: f32) -> bool {
        let Some(at) = self.ring.iter().position(|d| d.id == id) else {
            return false;
        };
        if fade > 0.0 {
            self.fade_at(at, fade, false);
        } else if let Some(gone) = self.ring.remove(at) {
            self.counts.forget(&gone);
        }
        true
    }

    /// Take every decal out, oldest first. Ids keep counting, and the budgets stay.
    pub(super) fn drain(&mut self) -> VecDeque<Decal> {
        self.counts = Counts::default();
        std::mem::take(&mut self.ring)
    }

    /// Keep only the decals `keep` accepts, in their order; the rest are dropped.
    fn retain(&mut self, keep: impl Fn(&Decal) -> bool) {
        let counts = &mut self.counts;
        self.ring.retain(|d| {
            let kept = keep(d);
            if !kept {
                counts.forget(d);
            }
            kept
        });
    }

    /// Put back the decals `saved` held (Stop's snapshot restore, #1518), as they
    /// were. The budgets in force stay, and ids keep counting past every one handed
    /// out since, so none is reused.
    pub fn restore(&mut self, saved: DecalSet) {
        let next_id = self.next_id.max(saved.next_id);
        (self.ring, self.counts, self.next_id) = (saved.ring, saved.counts, next_id);
    }

    /// Age every decal by `dt` sim seconds, then drop the ones whose lifetime ran
    /// out and the ones whose owner no longer exists (`exists`).
    pub fn tick(&mut self, dt: f32, exists: impl Fn(u32) -> bool) {
        for decal in &mut self.ring {
            decal.age += dt;
        }
        self.retain(|d| !d.expired() && d.owner.is_none_or(|o| exists(o.entity)));
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

    /// Drop every live decal (e.g. on level reset), or with `keep_persistent` every
    /// one but the [`Decal::persist`] ones.
    pub fn clear_decals(&mut self, keep_persistent: bool) {
        self.decals.retain(|d| keep_persistent && d.persist);
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "budget_tests.rs"]
mod budget_tests;

#[cfg(test)]
#[path = "headroom_tests.rs"]
mod headroom_tests;
