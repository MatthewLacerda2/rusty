//! src/app/animation/root_motion/ — root motion (#1745), Unity's *Apply Root
//! Motion*.
//!
//! One bone (the Animator's `root_bone`, else the skeleton's first root joint)
//! carries the character's travel. Each step, its **horizontal** displacement
//! and its **yaw** (the twist about the entity's up axis), both measured in the
//! skinned entity's space, are the step's [`RootMotion`]:
//! - per clip, over the playhead's travel before any wrap: a loop's wrap splits
//!   the travel at the seam and chains the pieces, so a looping walk keeps
//!   walking instead of snapping back, and a clamped clip stops at its end;
//! - each piece is measured in the frame the bone faced when it began, turned
//!   to its bind facing, so a clip that turns as it walks curves the entity;
//! - blend trees, crossfades and layers blend the deltas by the same weights
//!   they blend poses by (`blend`).
//!
//! When the animator applies it, [`RootBone::strip`] holds the bone at its
//! bind-pose horizontal position and yaw in the pose, so the travel shows up
//! only once: on the entity. Vertical motion stays in the pose. Pure functions
//! of (skin, clips, playheads): deterministic.

mod blend;

pub use blend::Outgoing;

use std::f32::consts::{PI, TAU};

use glam::{Mat4, Quat, Vec3};

use crate::asset::anim_data::AnimationClip;
use crate::asset::mesh_data::{JointTransform, SkinData};
use crate::components::{Bounds, RootMotion};

use super::clip::sample_joint;
use super::motion::Pose;

/// At most this many whole cycles of a loop count in a single step, so an
/// absurd speed on a short clip can't spin the walk (the events' own cap).
const MAX_CYCLES: usize = 16;

/// Where the bone stands in the skinned entity's space and which way it faces.
#[derive(Clone, Copy, Debug)]
struct Frame {
    position: Vec3,
    yaw: f32,
}

/// The bone root motion is read from, resolved against one skin.
#[derive(Clone, Debug)]
pub struct RootBone {
    slot: usize,
    /// The bone's joint chain from its skeleton root down to it.
    chain: Vec<usize>,
    bind: Frame,
}

impl RootBone {
    /// The bone named `name` in `skin` (empty: the first root joint). `None`
    /// when the skin has no such joint, or its parents loop.
    pub fn resolve(skin: &SkinData, name: &str) -> Option<Self> {
        let joints = skin.local_bind.len();
        let slot = match name {
            "" => skin.parents.iter().position(Option::is_none)?,
            _ => skin.names.iter().position(|n| n == name)?,
        };
        let mut chain = vec![slot];
        let mut at = slot;
        while let Some(parent) = skin.parents.get(at).copied().flatten() {
            if chain.len() > joints || parent >= joints {
                return None;
            }
            chain.push(parent);
            at = parent;
        }
        if slot >= joints {
            return None;
        }
        chain.reverse();
        let mut bone = Self {
            slot,
            chain,
            bind: Frame {
                position: Vec3::ZERO,
                yaw: 0.0,
            },
        };
        bone.bind = frame(bone.entity_matrix(skin, bone.chain.len(), |s| skin.local_bind[s]));
        Some(bone)
    }

    /// The joint slot of the bone.
    pub fn slot(&self) -> usize {
        self.slot
    }

    /// Whether `clip` keys any joint of the bone's chain.
    fn keyed_by(&self, clip: &AnimationClip) -> bool {
        self.chain.iter().any(|&s| {
            clip.tracks
                .get(s)
                .is_some_and(|t| !t.translation.times.is_empty() || !t.rotation.times.is_empty())
        })
    }

    /// The first `depth` joints of the chain composed in the skinned entity's
    /// space, each joint's local from `local`.
    fn entity_matrix(
        &self,
        skin: &SkinData,
        depth: usize,
        local: impl Fn(usize) -> JointTransform,
    ) -> Mat4 {
        let offset = skin.root_offset(self.chain[0]);
        self.chain[..depth]
            .iter()
            .fold(offset, |m, &s| m * local(s).matrix())
    }

    /// The bone's frame in `clip` at clip time `time`.
    fn at(&self, skin: &SkinData, clip: &AnimationClip, time: f32) -> Frame {
        let depth = self.chain.len();
        frame(self.entity_matrix(skin, depth, |s| sample_joint(skin, clip, s, time)))
    }

    /// The root motion of `clip` from clip time `a` to `b`: the horizontal
    /// travel in the frame the bone faced at `a`, turned to its bind facing.
    fn segment(&self, skin: &SkinData, clip: &AnimationClip, a: f32, b: f32) -> RootMotion {
        if a == b {
            return RootMotion::ZERO;
        }
        let (from, to) = (self.at(skin, clip, a), self.at(skin, clip, b));
        let mut travel = to.position - from.position;
        travel.y = 0.0;
        RootMotion {
            position: Quat::from_rotation_y(self.bind.yaw - from.yaw) * travel,
            yaw: wrap_angle(to.yaw - from.yaw),
        }
    }

    /// The root motion of `clip` over a playhead's travel `span` (before any
    /// wrap) within `bounds`, where one playhead unit is `scale` clip seconds
    /// (1 for a clip, its duration for a blend-tree leaf at a normalized phase).
    pub(super) fn travel(
        &self,
        skin: &SkinData,
        clip: &AnimationClip,
        bounds: Bounds,
        scale: f32,
        span: (f32, f32),
    ) -> RootMotion {
        let piece = |a: f32, b: f32| self.segment(skin, clip, a * scale, b * scale);
        match bounds {
            Bounds::Clamp(end) => piece(span.0.clamp(0.0, end), span.1.clamp(0.0, end)),
            Bounds::Open => piece(span.0, span.1),
            Bounds::Wrap(length) => wrapped(span, length, piece),
        }
    }

    /// Take the bone's horizontal travel and yaw out of `pose`: it stands at
    /// its bind-pose horizontal position, facing its bind yaw, with its
    /// height, tilt and scale as sampled. A pose that drives nothing on the
    /// bone's chain is left alone.
    pub fn strip(&self, skin: &SkinData, pose: &mut Pose) {
        if !self
            .chain
            .iter()
            .any(|&s| pose.get(s).is_some_and(Option::is_some))
        {
            return;
        }
        let local = |s: usize| pose.get(s).copied().flatten().unwrap_or(skin.local_bind[s]);
        let parent = self.entity_matrix(skin, self.chain.len() - 1, local);
        let posed = parent * local(self.slot).matrix();
        // lint: allow(world-space) a joint chain in the skinned entity's space, rig data, not the scene hierarchy
        let (scale, rotation, position) = posed.to_scale_rotation_translation();
        let rotation = Quat::from_rotation_y(self.bind.yaw - yaw_of(rotation)) * rotation;
        let position = Vec3::new(self.bind.position.x, position.y, self.bind.position.z);
        let held = Mat4::from_scale_rotation_translation(scale, rotation.normalize(), position);
        let held = parent.inverse() * held;
        // lint: allow(world-space) a joint chain in the skinned entity's space, rig data, not the scene hierarchy
        let (scale, rotation, translation) = held.to_scale_rotation_translation();
        if let Some(slot) = pose.get_mut(self.slot) {
            *slot = Some(JointTransform {
                translation,
                rotation,
                scale,
            });
        }
    }
}

/// A looping playhead's travel over `span`, split at each wrap of `length`:
/// to the seam, any whole cycles (at most [`MAX_CYCLES`]), then on from the
/// other end, each piece chained onto the facing the last left.
fn wrapped(span: (f32, f32), length: f32, piece: impl Fn(f32, f32) -> RootMotion) -> RootMotion {
    let (k0, k1) = ((span.0 / length).floor(), (span.1 / length).floor());
    let (a, b) = (span.0 - k0 * length, span.1 - k1 * length);
    if k0 == k1 {
        return piece(a, b);
    }
    let (seam, start) = if k1 > k0 {
        (length, 0.0)
    } else {
        (0.0, length)
    };
    let cycles = ((k1 - k0).abs() - 1.0).clamp(0.0, MAX_CYCLES as f32) as usize;
    let cycle = piece(start, seam);
    let mut out = piece(a, seam);
    for _ in 0..cycles {
        out = out.then(cycle);
    }
    out.then(piece(start, b))
}

fn frame(m: Mat4) -> Frame {
    // lint: allow(world-space) a joint chain in the skinned entity's space, rig data, not the scene hierarchy
    let (_, rotation, position) = m.to_scale_rotation_translation();
    Frame {
        position,
        yaw: yaw_of(rotation),
    }
}

/// The twist of `q` about +Y (its swing-twist split), radians in `(-π, π]`.
fn yaw_of(q: Quat) -> f32 {
    wrap_angle(2.0 * q.y.atan2(q.w))
}

/// `angle` wrapped into `(-π, π]`.
fn wrap_angle(angle: f32) -> f32 {
    let wrapped = (angle + PI).rem_euclid(TAU) - PI;
    if wrapped <= -PI {
        wrapped + TAU
    } else {
        wrapped
    }
}

#[cfg(test)]
mod blend_tests;
#[cfg(test)]
mod tests;
