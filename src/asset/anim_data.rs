//! src/asset/anim_data.rs — imported animation clips (issue #80).
//!
//! A [`AnimationClip`] is the pure-data product of a glTF `animation`: named, with
//! one [`JointTrack`] per *joint slot* it drives (the slot indexes the bound
//! [`super::SkinData`] joint arrays). Each track holds the keyframes for the joint's
//! translation / rotation / scale, ready for the deterministic keyframe sampler in
//! the sim path (`app::animation`) to interpolate.
//!
//! Like the rest of the importer this is decoupled from the renderer and the sim —
//! `glam` + plain `Vec`s only, never wgpu/egui/mlua. Clips are rehydrated from the
//! source file on scene load (never serialized), exactly like geometry and the bind
//! palette.

use glam::{Quat, Vec3};

/// How a sampler steps between two keyframes. glTF defines three modes; we support
/// the two that need no extra tangent data (`Step`, `Linear`) and fall back to
/// `Linear` for `CubicSpline`, which is a faithful-enough approximation without the
/// in/out tangents (kept deterministic, never wall-clock).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Interpolation {
    /// Hold the previous keyframe until the next (no blending).
    Step,
    /// Linearly interpolate (lerp for vectors, nlerp/slerp for rotations).
    #[default]
    Linear,
}

/// One scalar keyframe track: ascending key times paired with sampled values.
/// `times.len() == values.len()` by construction; an empty track contributes
/// nothing (the joint keeps its bind value).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Track<T> {
    pub times: Vec<f32>,
    pub values: Vec<T>,
    pub interpolation: Interpolation,
}

impl<T> Track<T> {
    pub fn is_empty(&self) -> bool {
        self.times.is_empty()
    }

    /// The track's last key time, or `0.0` when empty — used to derive the clip's
    /// overall duration.
    pub fn end_time(&self) -> f32 {
        self.times.last().copied().unwrap_or(0.0)
    }
}

/// The animated transform of one joint: independent T/R/S tracks. A path the clip
/// never animates stays empty, so the sampler falls back to the joint's bind value.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct JointTrack {
    pub translation: Track<Vec3>,
    pub rotation: Track<Quat>,
    pub scale: Track<Vec3>,
}

impl JointTrack {
    pub fn is_empty(&self) -> bool {
        self.translation.is_empty() && self.rotation.is_empty() && self.scale.is_empty()
    }

    fn end_time(&self) -> f32 {
        self.translation
            .end_time()
            .max(self.rotation.end_time())
            .max(self.scale.end_time())
    }
}

/// A named animation clip: one [`JointTrack`] per joint slot of the bound skin
/// (`tracks.len() == skin joint count`); slots the clip never touches hold an empty
/// track. `duration` is the latest key time across every track.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct AnimationClip {
    pub name: String,
    pub tracks: Vec<JointTrack>,
    pub duration: f32,
}

impl AnimationClip {
    /// (Re)compute `duration` as the maximum key time across all tracks.
    pub fn recompute_duration(&mut self) {
        self.duration = self
            .tracks
            .iter()
            .map(JointTrack::end_time)
            .fold(0.0_f32, f32::max);
    }
}
