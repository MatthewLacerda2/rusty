//! A ragdoll's runtime state, never saved. Where reactions overlap a bone takes the
//! largest weight: each eases continuously to 0, so their maximum does, with no jump.
use std::collections::{BTreeMap, BTreeSet};

use glam::{Quat, Vec3};

/// Seconds.
pub const REACT_DURATION: f32 = 0.4;
/// 1/s².
pub const REACT_STRENGTH: f32 = 5000.0;

/// The damping, in 1/s, that settles a drive of `strength` fastest without overshoot.
pub fn critical_damping(strength: f32) -> f32 {
    2.0 * strength.max(0.0).sqrt()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drive {
    pub strength: f32,
    pub damping: f32,
}

impl Drive {
    pub fn merged(self, strength: Option<f32>, damping: Option<f32>) -> Drive {
        let strength = strength.unwrap_or(self.strength).max(0.0);
        let damping = match damping {
            Some(d) => d.max(0.0),
            None if strength == self.strength && self.strength > 0.0 => self.damping,
            None => critical_damping(strength),
        };
        Drive { strength, damping }
    }
}

#[derive(Clone, Debug)]
struct Reaction {
    bones: Vec<u32>,
    elapsed: f32,
    duration: f32,
    drive: Drive,
}

impl Reaction {
    fn weight(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        let t = (self.elapsed / self.duration).clamp(0.0, 1.0);
        1.0 - t * t * (3.0 - 2.0 * t)
    }

    fn covers(&self, bone: u32) -> bool {
        self.bones.binary_search(&bone).is_ok()
    }
}

#[derive(Clone, Debug, Default)]
pub struct RagdollRuntime {
    animated: BTreeMap<u32, (Vec3, Quat)>,
    weights: BTreeMap<u32, f32>,
    reactions: BTreeMap<u32, Reaction>,
    reacting: BTreeSet<u32>,
    physics: BTreeMap<u32, (Vec3, Quat)>,
}

impl RagdollRuntime {
    pub fn is_idle(&self) -> bool {
        self.animated.is_empty()
    }

    pub fn track(&mut self, bone: u32, pose: (Vec3, Quat)) {
        self.animated.entry(bone).or_insert(pose);
    }

    /// Its blend weight stays, as Unreal's `PhysicsBlendWeight` stays on the body; it goes
    /// only with the ragdoll (`retain`).
    pub fn untrack(&mut self, bone: u32) {
        self.animated.remove(&bone);
        self.reacting.remove(&bone);
        self.physics.remove(&bone);
    }

    pub fn is_tracked(&self, bone: u32) -> bool {
        self.animated.contains_key(&bone)
    }

    pub fn note_physics(&mut self, bone: u32, pose: (Vec3, Quat)) {
        self.physics.insert(bone, pose);
    }

    pub fn take_physics(&mut self) -> BTreeMap<u32, (Vec3, Quat)> {
        std::mem::take(&mut self.physics)
    }

    pub fn note_animated(&mut self, bone: u32, pose: (Vec3, Quat)) {
        if let Some(held) = self.animated.get_mut(&bone) {
            *held = pose;
        }
    }

    pub fn animated(&self, bone: u32) -> Option<(Vec3, Quat)> {
        self.animated.get(&bone).copied()
    }

    pub fn has_weight(&self, bone: u32) -> bool {
        self.weights.contains_key(&bone)
    }

    pub fn set_weight(&mut self, bone: u32, weight: f32) {
        self.weights.insert(bone, weight);
    }

    pub fn weight(&self, bone: u32) -> f32 {
        match self.strongest(bone) {
            Some(r) => r.weight(),
            None => self.weights.get(&bone).copied().unwrap_or(1.0),
        }
    }

    pub fn drive_override(&self, bone: u32) -> Option<Drive> {
        self.strongest(bone).map(|r| r.drive)
    }

    pub fn is_reacting(&self, bone: u32) -> bool {
        self.reacting.contains(&bone)
    }

    fn strongest(&self, bone: u32) -> Option<&Reaction> {
        if !self.reacting.contains(&bone) {
            return None;
        }
        let covering = self.reactions.values().filter(|r| r.covers(bone));
        covering.reduce(|a, b| if b.weight() > a.weight() { b } else { a })
    }

    pub fn start(&mut self, root: u32, bones: Vec<u32>, owned: &[u32], timing: (f32, Drive)) {
        let (duration, drive) = timing;
        let reaction = Reaction {
            bones,
            elapsed: 0.0,
            duration,
            drive,
        };
        self.reactions.insert(root, reaction);
        self.reacting.extend(owned);
    }

    pub fn advance(&mut self, dt: f32) {
        for r in self.reactions.values_mut() {
            r.elapsed += dt;
        }
    }

    pub fn release(&mut self) -> Vec<u32> {
        self.reactions.retain(|_, r| r.elapsed < r.duration);
        let free: Vec<u32> = self
            .reacting
            .iter()
            .copied()
            .filter(|&b| !self.reactions.values().any(|r| r.covers(b)))
            .collect();
        for &b in &free {
            self.reacting.remove(&b);
        }
        free
    }

    pub fn forget(&mut self, bones: &[u32]) {
        self.reactions.retain(|root, _| !bones.contains(root));
        for b in bones {
            self.reacting.remove(b);
        }
    }

    pub fn retain(&mut self, keep: impl Fn(u32) -> bool) {
        self.animated.retain(|&b, _| keep(b));
        self.weights.retain(|&b, _| keep(b));
        self.reacting.retain(|&b| keep(b));
        self.physics.retain(|&b, _| keep(b));
        self.reactions.retain(|&b, _| keep(b));
        for r in self.reactions.values_mut() {
            r.bones.retain(|&b| keep(b));
        }
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
