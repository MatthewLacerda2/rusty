//! What a frame draws between fixed ticks: a moving entity blended between its last two, as
//! Unity's `RigidbodyInterpolation.Interpolate`.

use std::collections::{BTreeMap, BTreeSet};

use glam::{Mat4, Quat};

use rusty_components::transform::Pose;
use rusty_ecs::{Changes, Cursor, WorldPose};
use rusty_scene::{Camera, Scene, SceneId};

/// Metres in one tick (120 m/s at 60 Hz): a longer step is a teleport, drawn where it landed.
pub const JUMP_DISTANCE: f32 = 2.0;
/// Radians in one tick (90°): a larger turn is a teleport, drawn at its new pose.
pub const JUMP_TURN: f32 = std::f32::consts::FRAC_PI_2;

#[derive(Default)]
pub struct DrawPoses {
    tracked: Option<(SceneId, Cursor<WorldPose>)>,
    at_tick: BTreeMap<u32, Pose>,
    from: BTreeMap<u32, Pose>,
    alpha: f32,
    drawn: Option<(SceneId, BTreeSet<u32>)>,
}

impl DrawPoses {
    pub fn record(&mut self, scene: &Scene) {
        let world = &scene.world;
        let changes = match &mut self.tracked {
            Some((id, cursor)) if *id == scene.id() => world.changes(cursor),
            _ => {
                self.tracked = Some((scene.id(), world.change_cursor::<WorldPose>()));
                Changes::Rebuild
            }
        };
        let mut from = BTreeMap::new();
        match changes {
            Changes::Rebuild => {
                self.at_tick.clear();
                let drawn = world.ids_with_mesh().into_iter();
                let drawn = drawn.chain(world.ids_with_camera());
                let drawn = drawn.chain(world.ids_with_collider());
                for id in drawn.chain(world.ids_with_character_controller()) {
                    if let Some(pose) = world.world_pose(id) {
                        self.at_tick.insert(id, pose);
                    }
                }
            }
            Changes::Ids(ids) => {
                for id in ids {
                    let drawn = world.mesh(id).is_some()
                        || world.camera(id).is_some()
                        || world.has_collider(id)
                        || world.has_character_controller(id);
                    let now = world
                        .world_pose(id)
                        .filter(|_| drawn && !on_bone(scene, id));
                    let Some(now) = now else {
                        self.at_tick.remove(&id);
                        continue;
                    };
                    match self.at_tick.insert(id, now) {
                        Some(before) if !jumped(before, now) => {
                            from.insert(id, before);
                        }
                        _ => {}
                    }
                }
            }
        }
        self.from = from;
    }

    pub fn set_alpha(&mut self, alpha: f32) {
        self.alpha = alpha.clamp(0.0, 1.0);
    }

    pub fn clear(&mut self) {
        self.tracked = None;
        self.at_tick.clear();
        self.from.clear();
        self.alpha = 1.0;
    }

    fn blends(&self, scene: &Scene, id: u32) -> Option<(Pose, Pose)> {
        let (tracked, _) = self.tracked.as_ref()?;
        if *tracked != scene.id() || self.alpha >= 1.0 {
            return None;
        }
        let before = *self.from.get(&id)?;
        let now = scene.world.world_pose(id)?;
        (!jumped(before, now)).then_some((before, now))
    }

    pub fn world(&self, scene: &Scene, id: u32) -> Mat4 {
        let matrix = scene.world.local_to_world(id);
        match self.blends(scene, id) {
            Some((before, now)) => blend(before, now, self.alpha) * rigid(now).inverse() * matrix,
            None => matrix,
        }
    }

    pub fn pose(&self, scene: &Scene, id: u32) -> Option<Pose> {
        let live = scene.world.world_pose(id)?;
        Some(match self.blends(scene, id) {
            Some((before, now)) => Pose {
                pos: before.pos.lerp(now.pos, self.alpha),
                rot: before.rot.slerp(now.rot, self.alpha),
                scale: now.scale,
            },
            None => live,
        })
    }

    pub(crate) fn repose(&mut self, scene: &Scene) -> BTreeSet<u32> {
        let blended = self
            .from
            .keys()
            .filter(|&&id| self.blends(scene, id).is_some());
        let now: BTreeSet<u32> = blended.copied().collect();
        let mut ids = BTreeSet::new();
        match self.drawn.take() {
            Some((drawn, last)) if drawn == scene.id() => ids = last,
            other => self.drawn = other,
        }
        ids.extend(now.iter().copied());
        if !now.is_empty() {
            self.drawn = Some((scene.id(), now));
        }
        ids
    }

    pub fn pose_camera(&self, camera: &mut Camera, scene: &Scene, id: u32) {
        if self.blends(scene, id).is_some() {
            rusty_scene::camera::pose_at(camera, self.world(scene, id));
        }
    }
}

fn on_bone(scene: &Scene, id: u32) -> bool {
    let chain: Vec<u32> = scene.world.self_and_ancestors(id).collect();
    chain.iter().enumerate().skip(1).any(|(above, &owner)| {
        let mesh = scene.world.mesh(owner);
        let bones = mesh.as_ref().map(|m| &m.skeleton.bones);
        bones.is_some_and(|b| chain[..above].iter().any(|&c| b.contains(&Some(c))))
    })
}

fn jumped(before: Pose, now: Pose) -> bool {
    let far = before.pos.distance(now.pos) > JUMP_DISTANCE;
    let unknown = !before.pos.is_finite() || !now.pos.is_finite();
    far || unknown || before.rot.angle_between(now.rot) > JUMP_TURN
}

fn rigid(pose: Pose) -> Mat4 {
    Mat4::from_rotation_translation(pose.rot, pose.pos)
}

fn blend(before: Pose, now: Pose, alpha: f32) -> Mat4 {
    let rot: Quat = before.rot.slerp(now.rot, alpha);
    Mat4::from_rotation_translation(rot, before.pos.lerp(now.pos, alpha))
}

#[cfg(test)]
#[path = "bone_tests.rs"]
mod bone_tests;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
