mod blend;

use std::f32::consts::{PI, TAU};

use glam::{Mat4, Quat, Vec3};

use rusty_asset::anim_data::AnimationClip;
use rusty_asset::mesh_data::{JointTransform, SkinData};
use rusty_components::{root_chain, Bounds, RootMotion};

use super::clip::sample_joint;
use super::motion::Pose;

const MAX_CYCLES: usize = 16;

#[derive(Clone, Copy, Debug)]
struct Frame {
    position: Vec3,
    yaw: f32,
}

#[derive(Clone, Debug)]
pub struct RootBone {
    slot: usize,
    chain: Vec<usize>,
    model: Option<Mat4>,
    bind_position: Vec3,
    bind_rotation: Quat,
}

impl RootBone {
    pub fn resolve(skin: &SkinData, name: &str) -> Option<Self> {
        let chain = root_chain(skin, name)?;
        let slot = *chain.last()?;
        let mut bone = Self {
            slot,
            chain,
            model: skin.mesh_to_model_axes(),
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

    pub fn slot(&self) -> usize {
        self.slot
    }

    pub(in crate::animation) fn keyed_by(&self, clip: &AnimationClip) -> bool {
        self.chain.iter().any(|&s| {
            clip.tracks
                .get(s)
                .is_some_and(|t| !t.translation.times.is_empty() || !t.rotation.times.is_empty())
        })
    }

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

    fn at(&self, skin: &SkinData, clip: &AnimationClip, time: f32) -> Frame {
        let depth = self.chain.len();
        self.frame(self.model_matrix(skin, depth, |s| sample_joint(skin, clip, s, time)))
    }

    fn frame(&self, m: Mat4) -> Frame {
        // lint: allow(world-space) a joint chain in model space, rig data, not the scene hierarchy
        let (_, rotation, position) = m.to_scale_rotation_translation();
        Frame {
            position,
            yaw: self.yaw(rotation),
        }
    }

    fn yaw(&self, rotation: Quat) -> f32 {
        yaw_of(rotation * self.bind_rotation.inverse())
    }

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

fn yaw_of(q: Quat) -> f32 {
    wrap_angle(2.0 * q.y.atan2(q.w))
}

fn wrap_angle(angle: f32) -> f32 {
    let wrapped = (angle + PI).rem_euclid(TAU) - PI;
    if wrapped <= -PI {
        wrapped + TAU
    } else {
        wrapped
    }
}

#[cfg(test)]
mod armature_tests;
#[cfg(test)]
mod bind_tests;
#[cfg(test)]
mod blend_tests;
#[cfg(test)]
mod retarget_tests;
#[cfg(test)]
mod tests;
