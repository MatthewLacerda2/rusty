use glam::{Quat, Vec3};

use super::AnimatorComponent;
use rusty_asset::mesh_data::SkinData;

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
    /// Decided once here, so `animate` and the NavMeshAgent tick never disagree on who moves it.
    pub fn drives_root_motion(&self, skin: Option<&SkinData>) -> bool {
        self.apply_root_motion
            && self.is_running()
            && skin.is_some_and(|s| root_chain(s, &self.root_bone).is_some())
    }
}

/// Root motion's runtime bookkeeping on the animator; never saved.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RootMotionState {
    /// The last step's root motion, world space (`Animator.GetRootMotion`).
    /// Measured whether or not it is applied.
    pub last: RootMotion,
    /// What the entity really moved, world space, after walls and the navmesh (avoidance reads it).
    pub moved: Vec3,
    /// A CharacterController step under `min_move_distance`, carried on so a slow creep creeps.
    pub carried: Vec3,
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

    pub fn then(self, next: Self) -> Self {
        Self {
            position: self.position + Quat::from_rotation_y(self.yaw) * next.position,
            yaw: self.yaw + next.yaw,
        }
    }

    pub fn lerp(self, to: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            position: self.position.lerp(to.position, t),
            yaw: self.yaw + (to.yaw - self.yaw) * t,
        }
    }

    pub fn add_scaled(self, other: Self, weight: f32) -> Self {
        Self {
            position: self.position + other.position * weight,
            yaw: self.yaw + other.yaw * weight,
        }
    }

    pub fn is_zero(&self) -> bool {
        *self == Self::ZERO
    }
}
