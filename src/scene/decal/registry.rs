//! The scene's decals (#639): a spawn-ordered ring with a cap that fades the oldest
//! out instead of popping it, per-decal handles, and the fixed-tick ageing.

use std::collections::VecDeque;
use std::ops::Deref;

use glam::Vec3;

use super::{Decal, DecalOwner, DecalSpec};
use crate::scene::Scene;

/// Decals that stay whole at once. Bullet holes and scorch accumulate; past this
/// many, the oldest starts fading out ([`EVICTION_FADE`]) for each new one.
pub const MAX_DECALS: usize = 256;

/// Seconds an evicted decal takes to fade out, so the cap never pops a mark in view.
pub const EVICTION_FADE: f32 = 0.5;

/// Room past [`MAX_DECALS`] for decals still fading out. Only when that fills too
/// (more than `EVICTION_HEADROOM / EVICTION_FADE` = 128 stamps a second, sustained)
/// is the oldest dropped outright, so the frame's decal count stays bounded.
pub const EVICTION_HEADROOM: usize = 64;

/// The scene's live decals, oldest first (the order they blend in). Reads go
/// through [`Deref`] to the ring; every write is a method, so the cap holds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DecalSet {
    ring: VecDeque<Decal>,
    /// The next handle; never reset, so a stale id can never name a newer decal.
    next_id: u32,
}

impl Deref for DecalSet {
    type Target = VecDeque<Decal>;
    fn deref(&self) -> &Self::Target {
        &self.ring
    }
}

impl DecalSet {
    /// Add `decal` under a fresh id, making room per the cap. Returns its id.
    fn push(&mut self, mut decal: Decal) -> u32 {
        let whole = self.ring.iter().filter(|d| !d.retiring).count();
        if whole >= MAX_DECALS {
            if let Some(oldest) = self.ring.iter_mut().find(|d| !d.retiring) {
                oldest.retire(EVICTION_FADE);
            }
        }
        while self.ring.len() >= MAX_DECALS + EVICTION_HEADROOM {
            self.ring.pop_front();
        }
        self.next_id += 1;
        decal.id = self.next_id;
        self.ring.push_back(decal);
        self.next_id
    }

    /// Remove decal `id` now, or fade it out over `fade` seconds when `fade > 0`.
    /// `false` when no live decal has that id.
    pub fn remove(&mut self, id: u32, fade: f32) -> bool {
        let Some(at) = self.ring.iter().position(|d| d.id == id) else {
            return false;
        };
        if fade > 0.0 {
            self.ring[at].retire(fade);
        } else {
            self.ring.remove(at);
        }
        true
    }

    /// Drop every decal (a level reset, a scene load). Ids keep counting.
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
            // The live walk, not the per-frame store: a script may have moved the
            // owner earlier this tick.
            let world = self.compute_world_matrix(entity);
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
