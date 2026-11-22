use std::collections::BTreeMap;

use glam::{Quat, Vec3};

use super::runtime::{critical_damping, Drive, REACT_DURATION, REACT_STRENGTH};
use super::RAGDOLL_LAYER;
use crate::rules::joint::drive_value;
use crate::skeleton::HitboxOptions;
use crate::Scene;

pub type Velocity<'a> = &'a dyn Fn(u32) -> Option<(Vec3, Vec3)>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitReact {
    /// How long the weight takes to ease back to 0 (seconds).
    pub duration: f32,
    pub drive: Drive,
    pub up_to: Option<u32>,
}

impl HitReact {
    /// These options with every number finite and in range: a NaN or infinite
    /// duration takes the default, and each drive value takes the joint rule
    /// (`rules::joint::drive_value`: floored at 0, and 0 when NaN or infinite), so
    /// `math.huge` never reaches rapier.
    pub fn sanitized(&self) -> Self {
        let duration = match self.duration.is_finite() {
            true => self.duration.max(0.0),
            false => REACT_DURATION,
        };
        Self {
            duration,
            drive: Drive {
                strength: drive_value(self.drive.strength),
                damping: drive_value(self.drive.damping),
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
    pub fn ragdoll_body_at(&self, owner: u32, bone: u32) -> Option<u32> {
        let bodies = self.ragdoll_bones(owner);
        let mut up = std::iter::once(bone).chain(self.world.ancestors(bone));
        up.find(|b| bodies.binary_search(b).is_ok())
    }

    pub fn ragdoll_branch(&self, owner: u32, bone: u32) -> Vec<u32> {
        let bodies = self.ragdoll_bones(owner);
        let mut out: Vec<u32> = self.world.self_and_descendants(bone).collect();
        out.retain(|b| bodies.binary_search(b).is_ok());
        out.sort_unstable();
        out
    }

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

    pub fn set_drive(&mut self, owner: u32, bone: u32, given: [Option<f32>; 2]) -> usize {
        let mut n = 0;
        for b in self.ragdoll_branch(owner, bone) {
            if let Some(mut j) = self.world.joint_mut(b) {
                let own = Drive {
                    strength: j.drive_strength,
                    damping: j.drive_damping,
                };
                let drive = own.merged(given[0], given[1]);
                j.drive_strength = drive_value(drive.strength);
                j.drive_damping = drive_value(drive.damping);
                n += 1;
            }
            self.track_if_simulated(b);
        }
        n
    }

    pub fn track_driven_bone(&mut self, joint: u32) {
        if !self.ragdolls.is_tracked(joint) && self.is_bone(joint) {
            self.track_if_simulated(joint);
        }
    }

    fn track_if_simulated(&mut self, bone: u32) {
        if self.is_simulated_body(bone) {
            let pose = self.world.transform(bone).map(|t| (t.position, t.rotation));
            self.ragdolls.track(bone, pose.unwrap_or_default());
        }
    }

    pub(super) fn is_simulated_body(&self, bone: u32) -> bool {
        self.world
            .rigidbody(bone)
            .is_some_and(|rb| !rb.is_kinematic)
    }

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

    fn nearest(&self, bones: &[u32], point: Vec3) -> Option<u32> {
        let distance = |b: &u32| self.world.world_position(*b).distance_squared(point);
        bones
            .iter()
            .copied()
            .min_by(|a, b| distance(a).total_cmp(&distance(b)))
    }

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

    pub fn advance_reactions(&mut self, dt: f32) {
        self.ragdolls.take_physics();
        self.ragdolls.advance(dt);
    }

    pub(crate) fn unblend_ragdolls(&mut self) -> BTreeMap<u32, (Vec3, Quat)> {
        let blended = self.ragdolls.take_physics();
        for &b in blended.keys() {
            let animated = self.ragdolls.animated(b);
            if let (Some(pose), Some(mut t)) = (animated, self.world.transform_mut(b)) {
                (t.position, t.rotation) = pose;
            }
        }
        blended
    }

    pub(crate) fn reblend_ragdolls(&mut self, blended: BTreeMap<u32, (Vec3, Quat)>) {
        for (b, (pos, rot)) in blended {
            let w = self.ragdolls.weight(b);
            if let Some(mut t) = self.world.transform_mut(b) {
                t.position = t.position.lerp(pos, w);
                t.rotation = t.rotation.slerp(rot, w);
            }
            self.ragdolls.note_physics(b, (pos, rot));
        }
    }

    pub fn end_reactions(&mut self) {
        for b in self.ragdolls.release() {
            self.switch_body(b, None, false);
        }
    }
}
