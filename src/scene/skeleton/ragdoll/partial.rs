//! A ragdoll in part (#1895): one branch of the body on physics while the rest
//! stays animated, its joints driven toward the animated pose, its drawn pose a
//! blend of both, and the one-call hit reaction built from those three. Unreal's
//! `SetAllBodiesBelowSimulatePhysics`, `SetAllBodiesBelowPhysicsBlendWeight` and
//! physical animation in local space; the state they keep is `runtime`'s.
//!
//! A **branch** is a ragdoll bone and every ragdoll bone below it. A partly
//! simulated branch keeps its hitboxes on the `Hitbox` layer, which collides with
//! nothing, so a flinching arm can't catch on a door frame; only a full
//! `Enable` moves a body to the `Ragdoll` layer.

use std::collections::BTreeMap;

use glam::{Quat, Vec3};

use super::runtime::{critical_damping, Drive, REACT_DURATION, REACT_STRENGTH};
use super::RAGDOLL_LAYER;
use crate::scene::authoring::physics::joint as ops;
use crate::scene::skeleton::HitboxOptions;
use crate::scene::Scene;

/// A body's `(linear, angular)` velocity, as physics has it.
pub type Velocity<'a> = &'a dyn Fn(u32) -> Option<(Vec3, Vec3)>;

/// What `Ragdoll.HitReact` was asked for beyond the bone, impulse and point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitReact {
    /// How long the weight takes to ease back to 0 (seconds).
    pub duration: f32,
    /// The branch's drive while it reacts.
    pub drive: Drive,
    /// The bone whose branch reacts, instead of the default.
    pub up_to: Option<u32>,
}

impl HitReact {
    /// These options with every number finite and in range: a NaN or infinite
    /// duration takes the default, a drive the authoring floor at 0 (`math.huge`
    /// never reaches rapier).
    pub fn sanitized(&self) -> Self {
        let duration = match self.duration.is_finite() {
            true => self.duration.max(0.0),
            false => REACT_DURATION,
        };
        Self {
            duration,
            drive: Drive {
                strength: ops::drive_value(self.drive.strength),
                damping: ops::drive_value(self.drive.damping),
            },
            up_to: self.up_to,
        }
    }
}

impl Default for HitReact {
    fn default() -> Self {
        Self {
            duration: REACT_DURATION,
            drive: Drive {
                strength: REACT_STRENGTH,
                damping: critical_damping(REACT_STRENGTH),
            },
            up_to: None,
        }
    }
}

impl Scene {
    /// `owner`'s ragdoll bone at `bone`, or its nearest ancestor that is one.
    pub fn ragdoll_body_at(&self, owner: u32, bone: u32) -> Option<u32> {
        let bodies = self.ragdoll_bones(owner);
        let mut up = std::iter::once(bone).chain(self.world.ancestors(bone));
        up.find(|b| bodies.binary_search(b).is_ok())
    }

    /// The ragdoll bones of `owner` at and below `bone`, ascending.
    pub fn ragdoll_branch(&self, owner: u32, bone: u32) -> Vec<u32> {
        let bodies = self.ragdoll_bones(owner);
        let mut out: Vec<u32> = self.world.self_and_descendants(bone).collect();
        out.retain(|b| bodies.binary_search(b).is_ok());
        out.sort_unstable();
        out
    }

    /// Switch the branch at `bone` to physics (`on`, each body moving at the
    /// velocity its animation gave it) or back to the Animator, leaving the rest
    /// of the body as it is; how many bodies switched. Switching off moves a
    /// fully ragdolled branch's hitboxes back to the hitbox layer.
    pub fn set_simulated(&mut self, owner: u32, bone: u32, on: bool, velocity: Velocity) -> usize {
        let bones = self.ragdoll_branch(owner, bone);
        self.ragdolls.forget(&bones);
        let layers = (
            self.layers.index_of(RAGDOLL_LAYER),
            self.layers.index_of(&HitboxOptions::default().layer),
        );
        for &b in &bones {
            self.switch_body(b, on.then(|| velocity(b).unwrap_or_default()), true);
            if let (false, (Some(from), Some(to))) = (on, layers) {
                self.move_part_layers(b, from, to);
            }
        }
        bones.len()
    }

    /// Set the drive on every joint of the branch at `bone`; how many joints.
    /// A simulated bone given a drive starts tracking its animated pose, the
    /// drive's target.
    pub fn set_drive(&mut self, owner: u32, bone: u32, drive: Drive) -> usize {
        let mut n = 0;
        for b in self.ragdoll_branch(owner, bone) {
            if let Some(mut j) = self.world.joint_mut(b) {
                ops::set_drive_strength(&mut j, drive.strength);
                ops::set_drive_damping(&mut j, drive.damping);
                n += 1;
            }
            self.track_if_simulated(b);
        }
        n
    }

    /// Start tracking `bone`'s animated pose, from the pose it has now, when its
    /// body is simulated: a drive or a blend weight now needs it.
    fn track_if_simulated(&mut self, bone: u32) {
        if self.is_simulated_body(bone) {
            let pose = self.world.transform(bone).map(|t| (t.position, t.rotation));
            self.ragdolls.track(bone, pose.unwrap_or_default());
        }
    }

    /// Whether `bone`'s body is dynamic.
    pub(super) fn is_simulated_body(&self, bone: u32) -> bool {
        self.world
            .rigidbody(bone)
            .is_some_and(|rb| !rb.is_kinematic)
    }

    /// Set the blend weight (clamped to `0..=1`; NaN is ignored) of every bone
    /// of the branch at `bone`; how many bones.
    pub fn set_blend_weight(&mut self, owner: u32, bone: u32, weight: f32) -> usize {
        if weight.is_nan() {
            return 0;
        }
        let bones = self.ragdoll_branch(owner, bone);
        for &b in &bones {
            self.ragdolls.set_weight(b, weight.clamp(0.0, 1.0));
            self.track_if_simulated(b);
        }
        bones.len()
    }

    /// Start a hit reaction on `owner` hit at `bone` (world `point`), and return
    /// the body the impulse belongs on: `bone`'s, or its nearest ragdolled
    /// ancestor's, when that body is simulated; else the reacting branch's body
    /// nearest `point` (a pelvis shot whose branch starts below it, an `up_to`
    /// branch that doesn't hold the hit), since a kinematic body drops an
    /// impulse. `None` without a ragdoll. A body already simulated outside any
    /// reaction (`Enable`, `SetSimulated`) only takes the impulse.
    pub fn hit_react(
        &mut self,
        owner: u32,
        (bone, point): (u32, Vec3),
        opts: &HitReact,
        velocity: Velocity,
    ) -> Option<u32> {
        let hit = self.ragdoll_body_at(owner, bone)?;
        if self.is_simulated_body(hit) && !self.ragdolls.is_reacting(hit) {
            return Some(hit);
        }
        let up_to = opts.up_to.and_then(|b| self.ragdoll_body_at(owner, b));
        let root = up_to.unwrap_or_else(|| self.reaction_root(owner, hit, point));
        let bones = self.ragdoll_branch(owner, root);
        let mut owned = Vec::new();
        for &b in &bones {
            if !self.is_simulated_body(b) {
                self.switch_body(b, Some(velocity(b).unwrap_or_default()), true);
                owned.push(b);
            } else if self.ragdolls.is_reacting(b) {
                owned.push(b);
            }
        }
        let opts = opts.sanitized();
        let target = match self.is_simulated_body(hit) {
            true => hit,
            false => self.nearest(&bones, point).unwrap_or(root),
        };
        self.ragdolls
            .start(root, bones, &owned, (opts.duration, opts.drive));
        Some(target)
    }

    /// The bone of `bones` whose origin is nearest `point`; the first on a tie.
    fn nearest(&self, bones: &[u32], point: Vec3) -> Option<u32> {
        let distance = |b: &u32| self.world.world_position(*b).distance_squared(point);
        bones
            .iter()
            .copied()
            .min_by(|a, b| distance(a).total_cmp(&distance(b)))
    }

    /// The branch a hit on body `hit` moves by default: from its ragdolled
    /// parent, so a forearm shot moves the arm and a chest shot the torso; from
    /// `hit` itself when that parent is the root (the hips), which never reacts;
    /// and for a hit on the root, the root's child body nearest `point`.
    fn reaction_root(&self, owner: u32, hit: u32, point: Vec3) -> u32 {
        let parent = |b: u32| self.world.joint(b).and_then(|j| j.connected_body);
        match parent(hit) {
            Some(p) if parent(p).is_some() => p,
            Some(_) => hit,
            None => {
                let mut children = self.ragdoll_bones(owner);
                children.retain(|&b| parent(b) == Some(hit));
                self.nearest(&children, point).unwrap_or(hit)
            }
        }
    }

    /// One fixed tick of the hit reactions: age them by `dt`. Run before the
    /// ragdoll bones are posed, so a reaction's last tick poses at weight 0.
    pub fn advance_reactions(&mut self, dt: f32) {
        self.ragdolls.take_physics(); // last tick's blend is done with
        self.ragdolls.advance(dt);
    }

    /// Put every bone blended this tick back on its animated pose, so IK
    /// solves on the animation as it would with no physics in the way; the
    /// physics poses, for [`Scene::reblend_ragdolls`]. Run around IK only.
    pub(in crate::scene) fn unblend_ragdolls(&mut self) -> BTreeMap<u32, (Vec3, Quat)> {
        let blended = self.ragdolls.take_physics();
        for &b in blended.keys() {
            let animated = self.ragdolls.animated(b);
            if let (Some(pose), Some(mut t)) = (animated, self.world.transform_mut(b)) {
                (t.position, t.rotation) = pose;
            }
        }
        blended
    }

    /// Redo the blend of each bone [`Scene::unblend_ragdolls`] gave back, now
    /// between its IK'd animated pose and its physics pose, so a reaction on an
    /// aiming arm keeps the aim and ends with no snap.
    pub(in crate::scene) fn reblend_ragdolls(&mut self, blended: BTreeMap<u32, (Vec3, Quat)>) {
        for (b, (pos, rot)) in blended {
            let w = self.ragdolls.weight(b);
            if let Some(mut t) = self.world.transform_mut(b) {
                t.position = t.position.lerp(pos, w);
                t.rotation = t.rotation.slerp(rot, w);
            }
            self.ragdolls.note_physics(b, (pos, rot));
        }
    }

    /// Hand the bones no reaction holds any more back to the Animator: run after
    /// they are posed, so they leave physics on a tick drawn at weight 0.
    pub fn end_reactions(&mut self) {
        for b in self.ragdolls.release() {
            self.switch_body(b, None, false);
        }
    }
}
