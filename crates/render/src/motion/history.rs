use glam::Mat4;
use std::collections::HashMap;

use crate::gpu::draw_buffers::JointMatrix;
use crate::InstanceData;
use rusty_scene::{Scene, SceneId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HistoryKey {
    pub scene: SceneId,
    pub base: Option<u32>,
    pub playing: bool,
    pub cuts: u32,
}

impl HistoryKey {
    pub(crate) fn new(scene: &Scene, base: Option<u32>) -> Self {
        Self {
            scene: scene.id(),
            base,
            playing: scene.is_playing(),
            cuts: scene.history_cuts.of(base),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Pose {
    pub model: Mat4,
    pub palette: Vec<Mat4>,
}

#[derive(Default)]
pub(crate) struct MotionHistory {
    key: Option<HistoryKey>,
    cameras: HashMap<Option<u32>, Mat4>,
    prev: HashMap<u32, Pose>,
    next: HashMap<u32, Pose>,
    time: Option<f32>,
    cut: bool,
}

impl MotionHistory {
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

    /// Call after `begin`, which forgets the last frame's time on a cut.
    pub(crate) fn game_time(&mut self, now: f32) -> f32 {
        self.time.replace(now).unwrap_or(now)
    }

    pub(crate) fn cut(&self) -> bool {
        self.cut
    }

    pub(crate) fn pose(&self, id: u32) -> Option<&Pose> {
        self.prev.get(&id)
    }

    pub(crate) fn record(&mut self, id: u32, model: Mat4, palette: &[Mat4]) {
        let pose = Pose {
            model,
            palette: palette.to_vec(),
        };
        self.next.insert(id, pose);
    }

    pub(crate) fn end(&mut self) {
        std::mem::swap(&mut self.prev, &mut self.next);
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
}

pub(crate) fn track(
    history: &mut MotionHistory,
    id: u32,
    (model, palette): (Mat4, &[Mat4]),
    joints: &mut Vec<JointMatrix>,
    instance: &mut InstanceData,
) {
    if let Some(prev) = history.pose(id) {
        // +1: the upload puts the identity at element 0;
        // `vertex_to_world_prev` reads `bones[motion]`.
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
