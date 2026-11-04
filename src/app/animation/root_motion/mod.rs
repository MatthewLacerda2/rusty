//! src/app/animation/root_motion/ — root motion (#1745), Unity's *Apply Root
//! Motion*.
//!
//! One bone (the Animator's `root_bone`, else the skeleton's first root joint)
//! carries the character's travel. Each step, its **horizontal** displacement
//! and its **yaw** (the twist about the up axis), both measured in the model's
//! Y-up space ([`SkinData::mesh_to_model`], #1822: a Blender export's Z-up,
//! centimetre `Armature` above the mesh is undone, so its forward travel is not
//! mistaken for vertical), are the step's [`RootMotion`] (the `animate` system
//! carries it into the world through the skinned entity):
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
use crate::components::{root_chain, Bounds, RootMotion};

use super::clip::sample_joint;
use super::motion::Pose;

/// At most this many whole cycles of a loop count in a single step, so an
/// absurd speed on a short clip can't spin the walk (the events' own cap).
const MAX_CYCLES: usize = 16;

/// Where the bone stands in model space and which way it faces.
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
    /// Mesh-local space into model space, `None` when they coincide (a native
    /// Y-up rig), so such a rig's deltas are computed exactly as before.
    model: Option<Mat4>,
    /// Where the bone stands in the bind pose, in model space.
    bind_position: Vec3,
    /// Its bind rotation there: yaw is measured relative to it, so a bone
    /// bound flipped (a Z-up rig's hips) still has a twist to measure.
    bind_rotation: Quat,
}

impl RootBone {
    /// The bone named `name` in `skin` (empty: the first root joint). `None`
    /// when the skin has no such joint, or its parents loop
    /// ([`root_chain`]).
    pub fn resolve(skin: &SkinData, name: &str) -> Option<Self> {
        let chain = root_chain(skin, name)?;
        let slot = *chain.last()?;
        let mut bone = Self {
            slot,
            chain,
            model: Some(skin.mesh_to_model()).filter(|m| *m != Mat4::IDENTITY),
            bind_position: Vec3::ZERO,
            bind_rotation: Quat::IDENTITY,
        };
        let bind = bone.model_matrix(skin, bone.chain.len(), |s| skin.local_bind[s]);
        // lint: allow(world-space) a joint chain in model space, rig data, not the scene hierarchy
        let (_, rotation, position) = bind.to_scale_rotation_translation();
        bone.bind_position = position;
        bone.bind_rotation = rotation.normalize();
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

    /// The first `depth` joints of the chain composed in model space, each
    /// joint's local from `local`.
    fn model_matrix(
        &self,
        skin: &SkinData,
        depth: usize,
        local: impl Fn(usize) -> JointTransform,
    ) -> Mat4 {
        let offset = skin.root_offset(self.chain[0]);
        let offset = self.model.map_or(offset, |model| model * offset);
        self.chain[..depth]
            .iter()
            .fold(offset, |m, &s| m * local(s).matrix())
    }

    /// The bone's frame in `clip` at clip time `time`.
    fn at(&self, skin: &SkinData, clip: &AnimationClip, time: f32) -> Frame {
        let depth = self.chain.len();
        self.frame(self.model_matrix(skin, depth, |s| sample_joint(skin, clip, s, time)))
    }

    /// Where the bone stands under the model-space matrix `m`, and its yaw
    /// away from its bind facing.
    fn frame(&self, m: Mat4) -> Frame {
        // lint: allow(world-space) a joint chain in model space, rig data, not the scene hierarchy
        let (_, rotation, position) = m.to_scale_rotation_translation();
        Frame {
            position,
            yaw: self.yaw(rotation),
        }
    }

    /// The twist about +Y that turns the bone's bind rotation into `rotation`
    /// (applied in model space, after the bind rotation).
    fn yaw(&self, rotation: Quat) -> f32 {
        yaw_of(rotation * self.bind_rotation.inverse())
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
            position: Quat::from_rotation_y(-from.yaw) * travel,
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
        let parent = self.model_matrix(skin, self.chain.len() - 1, local);
        let posed = parent * local(self.slot).matrix();
        // lint: allow(world-space) a joint chain in model space, rig data, not the scene hierarchy
        let (scale, rotation, position) = posed.to_scale_rotation_translation();
        let rotation = Quat::from_rotation_y(-self.yaw(rotation)) * rotation;
        let position = Vec3::new(self.bind_position.x, position.y, self.bind_position.z);
        let held = Mat4::from_scale_rotation_translation(scale, rotation.normalize(), position);
        let held = parent.inverse() * held;
        // lint: allow(world-space) a joint chain in model space, rig data, not the scene hierarchy
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
mod bind_tests;
#[cfg(test)]
mod blend_tests;
#[cfg(test)]
mod retarget_tests;
#[cfg(test)]
mod tests;
