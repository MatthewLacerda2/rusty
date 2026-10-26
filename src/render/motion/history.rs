//! src/render/motion/history.rs — what one view remembers of last frame (#1435): each
//! stacked camera's view-projection and each drawn entity's pose.
//!
//! A frame reads last frame's poses and records its own beside them, so every camera
//! of a stack that draws one entity reprojects it from the same last-frame pose; the
//! frame's poses replace last frame's when the stack ends ([`MotionHistory::end`]).
//! An entity not drawn this frame (culled, hidden) has no pose next frame. Entity ids
//! are per scene, so the history is per view and starts over when the scene, the base
//! camera or the stack's size changes ([`HistoryKey`]).

use glam::Mat4;
use std::collections::HashMap;

use crate::render::gpu::draw_buffers::{push_palette, JointMatrix};
use crate::render::InstanceData;
use crate::scene::SceneId;

/// What a view's history is valid for; any change is a cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HistoryKey {
    pub scene: SceneId,
    /// The stack's base camera entity (`None`: the editor's free camera, a view with
    /// no scene camera). Another one is a camera switch.
    pub base: Option<u32>,
    /// How many cameras the stack holds.
    pub cameras: usize,
}

/// One entity's pose as drawn: its world matrix and its joint palette (empty when
/// it has no skin).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Pose {
    pub model: Mat4,
    pub palette: Vec<Mat4>,
}

#[derive(Default)]
pub(crate) struct MotionHistory {
    key: Option<HistoryKey>,
    cameras: Vec<Mat4>,
    prev: HashMap<u32, Pose>,
    next: HashMap<u32, Pose>,
}

impl MotionHistory {
    /// Open a frame of `key`'s stack, whose cameras draw through `view_projs`: a
    /// changed key forgets everything first. Returns each camera's last-frame matrix,
    /// its current one where it has none, and keeps the current ones for next frame.
    pub(crate) fn begin(&mut self, key: HistoryKey, view_projs: &[Mat4]) -> Vec<Mat4> {
        if self.key != Some(key) {
            self.clear();
            self.key = Some(key);
        }
        self.next.clear();
        let prev = view_projs
            .iter()
            .enumerate()
            .map(|(i, now)| self.cameras.get(i).copied().unwrap_or(*now))
            .collect();
        self.cameras = view_projs.to_vec();
        prev
    }

    /// Entity `id`'s pose last frame, if it was drawn then.
    pub(crate) fn pose(&self, id: u32) -> Option<&Pose> {
        self.prev.get(&id)
    }

    /// Record entity `id`'s pose this frame.
    pub(crate) fn record(&mut self, id: u32, model: Mat4, palette: &[Mat4]) {
        let pose = Pose {
            model,
            palette: palette.to_vec(),
        };
        self.next.insert(id, pose);
    }

    /// Close the frame: its poses become last frame's.
    pub(crate) fn end(&mut self) {
        self.prev = std::mem::take(&mut self.next);
    }

    /// Forget every pose and camera: the next frame is a cut.
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Give `instance` entity `id`'s last-frame pose from `history` and record this
/// frame's (`model`, `palette`, already at `bone_base` in `joints`). Last frame's
/// palette is appended to `joints` beside this frame's; one of another length (the
/// mesh changed) is not, and the vertex keeps this frame's skin under last frame's
/// world matrix.
pub(crate) fn track(
    history: &mut MotionHistory,
    id: u32,
    (model, palette): (Mat4, &[Mat4]),
    bone_base: u32,
    joints: &mut Vec<JointMatrix>,
    instance: &mut InstanceData,
) {
    if let Some(prev) = history.pose(id) {
        instance.prev_model_matrix = prev.model.to_cols_array();
        instance.prev_bone_base = match prev.palette.len() == palette.len() {
            true => push_palette(joints, &prev.palette),
            false => bone_base,
        };
        instance.motion = 1;
    }
    history.record(id, model, palette);
}
