//! src/components/animator/root_motion.rs — one step's root motion (#1745).
//!
//! What the root bone's horizontal travel and turn amount to over a fixed step,
//! Unity's `Animator.deltaPosition` / `deltaRotation`. The sampler builds one per
//! clip and blends them by weight the way it blends poses; the `animate` system
//! turns the result into world space and moves the entity by it.

use glam::{Quat, Vec3};

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
