use std::collections::{BTreeSet, VecDeque};
use std::ops::Deref;

use glam::Vec3;

use super::counts::Counts;
use super::{Decal, DecalOwner, DecalSpec};
use crate::Scene;
use rusty_core::settings::decals::DecalLimits;

/// Seconds an evicted decal fades over, so the cap never pops a mark in view.
pub const EVICTION_FADE: f32 = 0.5;

/// Spawn evictions fading at once (128 a second at `EVICTION_FADE`); past it the oldest is
/// dropped outright.
pub const EVICTION_HEADROOM: usize = 64;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DecalSet {
    ring: VecDeque<Decal>,
    /// Reset only by a new scene (`Scene::clear`), so a stale id never names a newer decal.
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
    pub fn limits(&self) -> DecalLimits {
        self.limits
    }

    fn budget(&self, bucket: Option<u32>) -> u32 {
        bucket.map_or(self.limits.max_world, |_| self.limits.max_per_owner)
    }

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

    fn push(&mut self, mut decal: Decal) -> u32 {
        self.next_id += 1;
        decal.id = self.next_id;
        self.insert(decal);
        self.next_id
    }

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

    fn evict_oldest(&mut self, pick: impl Fn(&Decal) -> bool) -> bool {
        let at = self.ring.iter().position(|d| !d.retiring && pick(d));
        at.inspect(|&at| self.fade_at(at, EVICTION_FADE, true))
            .is_some()
    }

    fn fade_at(&mut self, at: usize, seconds: f32, evicted: bool) {
        let d = &mut self.ring[at];
        if !d.retiring {
            self.counts.sub(d.bucket());
            d.evicted = evicted;
            self.counts.evicting += usize::from(evicted);
        }
        d.retire(seconds);
    }

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

    pub fn set_material(&mut self, id: u32, material: Option<String>) -> bool {
        let decal = self.ring.iter_mut().find(|d| d.id == id);
        decal.map(|d| d.material = material).is_some()
    }

    pub(super) fn drain(&mut self) -> VecDeque<Decal> {
        self.counts = Counts::default();
        std::mem::take(&mut self.ring)
    }

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

    pub(crate) fn take_owned(&mut self, owners: &BTreeSet<u32>) -> Vec<Decal> {
        let owned = |d: &Decal| d.owner.is_some_and(|o| owners.contains(&o.entity));
        let taken: Vec<Decal> = self.ring.iter().filter(|d| owned(d)).cloned().collect();
        self.retain(|d| !owned(d));
        taken
    }

    /// Undoes the owners' Destroy: each back in its blend-order place, no budget applied.
    pub(crate) fn put_back(&mut self, decals: Vec<Decal>) {
        for decal in decals {
            if self.ring.iter().any(|d| d.id == decal.id) {
                continue;
            }
            if decal.retiring {
                self.counts.evicting += usize::from(decal.evicted);
            } else {
                self.counts.add(decal.bucket());
            }
            // The ring's spawn order is id order.
            let at = self.ring.iter().position(|d| d.id > decal.id);
            self.ring.insert(at.unwrap_or(self.ring.len()), decal);
        }
    }

    /// Stop's snapshot restore: ids count on past every one handed out since.
    pub fn restore(&mut self, saved: DecalSet) {
        let next_id = self.next_id.max(saved.next_id);
        (self.ring, self.counts, self.next_id) = (saved.ring, saved.counts, next_id);
    }

    /// `dt` in sim seconds.
    pub fn tick(&mut self, dt: f32, exists: impl Fn(u32) -> bool) {
        for decal in &mut self.ring {
            decal.age += dt;
        }
        self.retain(|d| !d.expired() && d.owner.is_none_or(|o| exists(o.entity)));
    }
}

impl Scene {
    pub fn spawn_decal(&mut self, point: Vec3, normal: Vec3, spec: DecalSpec) -> Option<u32> {
        let owner = spec.owner;
        let mut decal = Decal::from_hit(point, normal, spec);
        if let Some(entity) = owner {
            if !self.world.contains(entity) {
                return None;
            }
            let world = self.world.local_to_world(entity);
            // A zero-scale owner has no space for the box: it is stamped unowned.
            if world.determinant().abs() > f32::EPSILON {
                let local = world.inverse() * decal.model_matrix();
                decal.owner = Some(DecalOwner { entity, local });
            }
        }
        Some(self.decals.push(decal))
    }

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
