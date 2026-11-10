//! src/render/motion/history.rs — what one view remembers of last frame (#1435): each
//! stacked camera's view-projection and each drawn entity's pose.
//!
//! A frame reads last frame's poses and records its own beside them, so every camera
//! of a stack that draws one entity reprojects it from the same last-frame pose; the
//! frame's poses replace last frame's when the stack ends ([`MotionHistory::end`]).
//! An entity not drawn this frame (culled, hidden) has no pose next frame. Entity ids
//! are per scene, so the history is per view and starts over when the scene, the base
//! camera or the play state changes, or the base camera is cut ([`HistoryKey`]).
//! Each camera's matrix is kept by its entity, not its place in the stack, so a camera swapped in at the same place
//! (a weapon switch's viewmodel camera) starts with no last frame.

use glam::Mat4;
use std::collections::HashMap;

use crate::render::gpu::draw_buffers::JointMatrix;
use crate::render::InstanceData;
use crate::scene::{Scene, SceneId};

/// What a view's history is valid for; any change is a cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HistoryKey {
    pub scene: SceneId,
    /// The stack's base camera entity (`None` when the scene has no active screen
    /// camera). Another one is a camera switch.
    pub base: Option<u32>,
    /// Whether the scene is in Play. Play and Stop are cuts, as Unity's scene reload
    /// is: Stop restores the edit scene under the same id, and its first frame must
    /// not reproject from the last Play pose.
    pub playing: bool,
    /// The base camera's cut count (`scene::camera::HistoryCuts`, #2003): a
    /// `Camera.ResetHistory` changes it, so the next frame starts over.
    pub cuts: u32,
}

impl HistoryKey {
    /// The key of a view drawing `scene` from base camera `base`.
    pub(crate) fn new(scene: &Scene, base: Option<u32>) -> Self {
        Self {
            scene: scene.id(),
            base,
            playing: scene.is_playing(),
            cuts: scene.history_cuts.of(base),
        }
    }
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
    /// Each stacked camera's view-projection last frame, by entity (`None`: a camera
    /// with no entity, the editor's free camera).
    cameras: HashMap<Option<u32>, Mat4>,
    prev: HashMap<u32, Pose>,
    next: HashMap<u32, Pose>,
    /// The game time the last frame drew at (#1524).
    time: Option<f32>,
    /// Whether this frame's [`MotionHistory::begin`] found no last frame (#1702).
    cut: bool,
}

impl MotionHistory {
    /// Open a frame of `key`'s stack, whose cameras (entity, view-projection) are
    /// `cameras`: a changed key forgets everything first. Returns each camera's
    /// last-frame matrix, its current one where it has none, and keeps the current
    /// ones for next frame.
    pub(crate) fn begin(&mut self, key: HistoryKey, cameras: &[(Option<u32>, Mat4)]) -> Vec<Mat4> {
        if self.key != Some(key) {
            self.clear();
            self.key = Some(key);
        }
        self.next.clear();
        self.cut = self.cameras.is_empty();
        let prev = cameras
            .iter()
            .map(|(id, now)| self.cameras.get(id).copied().unwrap_or(*now))
            .collect();
        self.cameras.clear();
        self.cameras.extend(cameras.iter().copied());
        prev
    }

    /// Last frame's game time, `now` where there was no last frame; keeps `now` for
    /// next frame. Call after [`MotionHistory::begin`], which forgets it on a cut.
    pub(crate) fn game_time(&mut self, now: f32) -> f32 {
        self.time.replace(now).unwrap_or(now)
    }

    /// Whether this frame is a cut: its stack had no last frame (the view's first,
    /// a changed [`HistoryKey`], a [`MotionHistory::clear`]). TAA drops its history
    /// on one (#1702).
    pub(crate) fn cut(&self) -> bool {
        self.cut
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

    /// Close the frame: its poses become last frame's. The old map is kept, to be
    /// cleared and refilled next frame without allocating.
    pub(crate) fn end(&mut self) {
        std::mem::swap(&mut self.prev, &mut self.next);
    }

    /// Forget every pose and camera: the next frame is a cut.
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Give `instance` entity `id`'s last-frame pose from `history` and record this
/// frame's (`model`, `palette`). Last frame's world matrix is appended to `joints`,
/// where `instance.motion` names it, and a skinned instance's last-frame palette
/// right after it (`forward/bindings.wgsl`'s `vertex_to_world_prev`); one of another
/// length (the mesh changed) is replaced by this frame's, so the vertex keeps this
/// frame's skin under last frame's world matrix. A view without motion vectors never
/// calls this and pays for none of it.
pub(crate) fn track(
    history: &mut MotionHistory,
    id: u32,
    (model, palette): (Mat4, &[Mat4]),
    joints: &mut Vec<JointMatrix>,
    instance: &mut InstanceData,
) {
    if let Some(prev) = history.pose(id) {
        // Element 0 of the uploaded array is the identity, so this starts one later.
        instance.motion = joints.len() as u32 + 1;
        joints.push(prev.model.to_cols_array());
        if !palette.is_empty() {
            let skin = match prev.palette.len() == palette.len() {
                true => &prev.palette[..],
                false => palette,
            };
            joints.extend(skin.iter().map(Mat4::to_cols_array));
        }
    }
    history.record(id, model, palette);
}
