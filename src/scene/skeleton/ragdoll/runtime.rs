//! A ragdoll's runtime state (#1895): what is not a component, so never saved.
//!
//! - **The animated pose** of every simulated ragdoll bone: the local pose its
//!   Animator sampled this tick (`note_animated`, from the animate system), or,
//!   for a bone the Animator doesn't key, the pose it had when it went simulated.
//!   It is the drives' target and the blend's animated side.
//! - **Blend weights** (`Ragdoll.SetBlendWeight`, Unreal's `PhysicsBlendWeight`):
//!   how much of a simulated bone's drawn pose is physics; absent is `1`.
//! - **Hit reactions** (`Ragdoll.HitReact`), keyed by the root bone of the branch
//!   they simulate. A reaction's weight eases from 1 to 0 over its duration; the
//!   bones it switched to physics (`reacting`) go back to the Animator once no
//!   reaction covers them. Where reactions overlap, the **outermost** one (the
//!   root nearest the hips) decides a bone's weight and drive, as Unreal's
//!   `SetAllBodiesBelowPhysicsBlendWeight` on a parent overrides its children.
//!
//! Every map is ordered, so the state reads the same way every run.

use std::collections::{BTreeMap, BTreeSet};

use glam::{Quat, Vec3};

/// A hit reaction's length when the call names none (seconds).
pub const REACT_DURATION: f32 = 0.4;
/// A hit reaction's drive strength when the call names none (1/s²).
pub const REACT_STRENGTH: f32 = 500.0;

/// The damping that settles a drive of `strength` fastest without overshoot.
pub fn critical_damping(strength: f32) -> f32 {
    2.0 * strength.max(0.0).sqrt()
}

/// A joint drive: its strength (1/s²) and damping (1/s).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drive {
    pub strength: f32,
    pub damping: f32,
}

/// One running hit reaction.
#[derive(Clone, Debug)]
struct Reaction {
    /// How many ancestors its root has: the smallest is the outermost.
    depth: usize,
    /// The ragdoll bones of its branch, ascending.
    bones: Vec<u32>,
    elapsed: f32,
    duration: f32,
    drive: Drive,
}

impl Reaction {
    /// 1 at the start, easing (smoothstep) to 0 at `duration`.
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

/// The ragdolls' runtime state. See the module docs.
#[derive(Clone, Debug, Default)]
pub struct RagdollRuntime {
    animated: BTreeMap<u32, (Vec3, Quat)>,
    weights: BTreeMap<u32, f32>,
    reactions: BTreeMap<u32, Reaction>,
    reacting: BTreeSet<u32>,
}

impl RagdollRuntime {
    /// Whether no bone is tracked: the animate hook's early out.
    pub fn is_idle(&self) -> bool {
        self.animated.is_empty()
    }

    /// Start tracking `bone`'s animated pose at `pose`, unless it is tracked.
    pub fn track(&mut self, bone: u32, pose: (Vec3, Quat)) {
        self.animated.entry(bone).or_insert(pose);
    }

    /// Forget everything about `bone`: it is back on the Animator, or gone.
    pub fn untrack(&mut self, bone: u32) {
        self.animated.remove(&bone);
        self.weights.remove(&bone);
        self.reacting.remove(&bone);
    }

    /// The Animator sampled `bone` at `pose` this tick; kept when it is tracked.
    pub fn note_animated(&mut self, bone: u32, pose: (Vec3, Quat)) {
        if let Some(held) = self.animated.get_mut(&bone) {
            *held = pose;
        }
    }

    /// `bone`'s animated local pose, when it is tracked.
    pub fn animated(&self, bone: u32) -> Option<(Vec3, Quat)> {
        self.animated.get(&bone).copied()
    }

    /// Set `bone`'s explicit blend weight (`0..=1`).
    pub fn set_weight(&mut self, bone: u32, weight: f32) {
        self.weights.insert(bone, weight);
    }

    /// How much of `bone`'s drawn pose is physics: its outermost reaction's
    /// weight while a reaction holds it, else its explicit weight, else 1.
    pub fn weight(&self, bone: u32) -> f32 {
        match self.outermost(bone) {
            Some(r) => r.weight(),
            None => self.weights.get(&bone).copied().unwrap_or(1.0),
        }
    }

    /// The drive a reaction holding `bone` puts on its joint, over the joint's own.
    pub fn drive_override(&self, bone: u32) -> Option<Drive> {
        self.outermost(bone).map(|r| r.drive)
    }

    /// Whether a reaction switched `bone` to physics and still holds it.
    pub fn is_reacting(&self, bone: u32) -> bool {
        self.reacting.contains(&bone)
    }

    /// The outermost reaction holding `bone`, when one does.
    fn outermost(&self, bone: u32) -> Option<&Reaction> {
        if !self.reacting.contains(&bone) {
            return None;
        }
        let covering = self.reactions.values().filter(|r| r.covers(bone));
        covering.min_by_key(|r| r.depth)
    }

    /// Start (or restart) the reaction rooted at `root`: `bones` is its branch,
    /// `owned` the bones it holds (it switched them to physics, or another
    /// reaction had).
    pub fn start(
        &mut self,
        root: u32,
        depth: usize,
        bones: Vec<u32>,
        owned: &[u32],
        timing: (f32, Drive),
    ) {
        let (duration, drive) = timing;
        let reaction = Reaction {
            depth,
            bones,
            elapsed: 0.0,
            duration,
            drive,
        };
        self.reactions.insert(root, reaction);
        self.reacting.extend(owned);
    }

    /// Advance every reaction by `dt` seconds.
    pub fn advance(&mut self, dt: f32) {
        for r in self.reactions.values_mut() {
            r.elapsed += dt;
        }
    }

    /// Drop the reactions whose weight reached 0, and return the bones no
    /// reaction holds any more, ascending: the ones to hand back to the Animator.
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

    /// Drop the reactions rooted in `bones` and stop them holding `bones`: a
    /// script switched those bones itself (`SetSimulated`, `Enable`, `Disable`).
    pub fn forget(&mut self, bones: &[u32]) {
        self.reactions.retain(|root, _| !bones.contains(root));
        for b in bones {
            self.reacting.remove(b);
        }
    }

    /// Keep only the state of the bones `keep` says, as a scene load does.
    pub fn retain(&mut self, keep: impl Fn(u32) -> bool) {
        self.animated.retain(|&b, _| keep(b));
        self.weights.retain(|&b, _| keep(b));
        self.reacting.retain(|&b| keep(b));
        self.reactions.retain(|&b, _| keep(b));
        for r in self.reactions.values_mut() {
            r.bones.retain(|&b| keep(b));
        }
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
