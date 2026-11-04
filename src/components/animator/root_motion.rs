//! src/components/animator/root_motion.rs — one step's root motion (#1745).
//!
//! What the root bone's horizontal travel and turn amount to over a fixed step,
//! Unity's `Animator.deltaPosition` / `deltaRotation`. The sampler builds one per
//! clip and blends them by weight the way it blends poses; the `animate` system
//! turns the result into world space and moves the entity by it.
//!
//! Whether root motion moves an entity at all is decided here, once
//! ([`AnimatorComponent::drives_root_motion`]), so the `animate` system and the
//! NavMeshAgent tick can never disagree about who moves it.

use glam::{Quat, Vec3};

use super::AnimatorComponent;
use crate::asset::mesh_data::SkinData;

/// The joint chain from the skeleton root down to the bone named `name` in
/// `skin` (empty: the first root joint), root first. `None` when the skin has
/// no such joint, or its parents loop.
pub fn root_chain(skin: &SkinData, name: &str) -> Option<Vec<usize>> {
    let joints = skin.local_bind.len();
    let slot = match name {
        "" => skin.parents.iter().position(Option::is_none)?,
        _ => skin.names.iter().position(|n| n == name)?,
    };
    if slot >= joints {
        return None;
    }
    let mut chain = vec![slot];
    let mut at = slot;
    while let Some(parent) = skin.parents.get(at).copied().flatten() {
        if chain.len() > joints || parent >= joints {
            return None;
        }
        chain.push(parent);
        at = parent;
    }
    chain.reverse();
    Some(chain)
}

impl AnimatorComponent {
    /// Whether root motion moves this animator's entity this tick: *Apply Root
    /// Motion* is on, the animator is running, and its root bone resolves in
    /// `skin` (the entity's mesh). Otherwise the entity is left to whatever
    /// else moves it: a NavMeshAgent with root motion on but a stopped
    /// animator, a misspelled bone or no skin moves itself.
    pub fn drives_root_motion(&self, skin: Option<&SkinData>) -> bool {
        self.apply_root_motion
            && self.is_running()
            && skin.is_some_and(|s| root_chain(s, &self.root_bone).is_some())
    }
}

/// Root motion's runtime bookkeeping on the animator (#1745); never saved.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RootMotionState {
    /// The last step's root motion, world space (`Animator.GetRootMotion`).
    /// Measured whether or not it is applied.
    pub last: RootMotion,
    /// What the entity really moved by last step, world space: the root
    /// motion after walls, the navmesh and links had their say (what a
    /// root-moved NavMeshAgent reports to its neighbours' avoidance).
    pub moved: Vec3,
    /// Travel a CharacterController hasn't taken yet: a step below its
    /// `min_move_distance` is carried into the next instead of dropped, so a
    /// slow creep still creeps.
    pub carried: Vec3,
    /// The root bone name already warned about as missing from the skeleton.
    pub warned_bone: Option<String>,
}

/// A step's root motion: the horizontal displacement (`y` is always 0) and the
/// turn about the up axis, radians, counter-clockwise seen from above.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RootMotion {
    pub position: Vec3,
    pub yaw: f32,
}

impl RootMotion {
    pub const ZERO: Self = Self {
        position: Vec3::ZERO,
        yaw: 0.0,
    };

    /// `self` followed by `next`, where `next` was measured in the frame `self`
    /// left the mover facing: its displacement is turned by `self`'s yaw.
    pub fn then(self, next: Self) -> Self {
        Self {
            position: self.position + Quat::from_rotation_y(self.yaw) * next.position,
            yaw: self.yaw + next.yaw,
        }
    }

    /// Blend toward `to` by `t` in `[0, 1]`, displacement and turn alike.
    pub fn lerp(self, to: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            position: self.position.lerp(to.position, t),
            yaw: self.yaw + (to.yaw - self.yaw) * t,
        }
    }

    /// `self` plus `other` scaled by `weight` (an additive layer, a blend-tree
    /// leaf's share).
    pub fn add_scaled(self, other: Self, weight: f32) -> Self {
        Self {
            position: self.position + other.position * weight,
            yaw: self.yaw + other.yaw * weight,
        }
    }

    /// True when the step neither moves nor turns.
    pub fn is_zero(&self) -> bool {
        *self == Self::ZERO
    }
}
