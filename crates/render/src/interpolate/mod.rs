//! What a frame draws between fixed ticks: a moving entity blended between its last two
//! poses at any speed (Unity's `RigidbodyInterpolation.Interpolate`). A pose *set*
//! (`World::cut_pose`), or written between ticks, is drawn where it landed.

use std::collections::{BTreeMap, BTreeSet};

use glam::{Mat4, Quat};

use rusty_components::transform::Pose;
use rusty_ecs::{Changes, Cursor, Cut, World, WorldPose};
use rusty_scene::{Camera, Scene, SceneId};

struct Tracked {
    scene: SceneId,
    poses: Cursor<WorldPose>,
    cuts: Cursor<Cut>,
}

#[derive(Default)]
pub struct DrawPoses {
    tracked: Option<Tracked>,
    at_tick: BTreeMap<u32, Pose>,
    from: BTreeMap<u32, Pose>,
    alpha: f32,
    drawn: Option<(SceneId, BTreeSet<u32>)>,
}

impl DrawPoses {
    pub fn record(&mut self, scene: &Scene) {
        let (moved, cuts) = self.take(scene);
        let mut from = moved.unwrap_or_default();
        match cuts {
            Changes::Rebuild => from.clear(),
            Changes::Ids(ids) => from.retain(|id, _| ids.binary_search(id).is_err()),
        }
        self.from = from;
    }

    /// Before a frame's ticks: a pose written since the last (the console, a gizmo) is
    /// where the next tick blends from.
    pub fn settle(&mut self, scene: &Scene) {
        if self.tracked.as_ref().is_none_or(|t| t.scene != scene.id()) {
            return;
        }
        match self.take(scene).0 {
            None => self.from.clear(),
            Some(written) => self.from.retain(|id, _| !written.contains_key(id)),
        }
    }

    fn take(&mut self, scene: &Scene) -> (Option<BTreeMap<u32, Pose>>, Changes) {
        let world = &scene.world;
        let (changes, cuts) = match &mut self.tracked {
            Some(t) if t.scene == scene.id() => {
                (world.changes(&mut t.poses), world.changes(&mut t.cuts))
            }
            _ => {
                self.tracked = Some(Tracked {
                    scene: scene.id(),
                    poses: world.change_cursor::<WorldPose>(),
                    cuts: world.change_cursor::<Cut>(),
                });
                (Changes::Rebuild, Changes::Rebuild)
            }
        };
        let Changes::Ids(ids) = changes else {
            self.at_tick.clear();
            let drawn = world.ids_with_mesh().into_iter();
            let drawn = drawn.chain(world.ids_with_camera());
            let drawn = drawn.chain(world.ids_with_collider());
            for id in drawn.chain(world.ids_with_character_controller()) {
                if let Some(pose) = world.world_pose(id) {
                    self.at_tick.insert(id, pose);
                }
            }
            return (None, cuts);
        };
        let mut from = BTreeMap::new();
        for id in ids {
            let now = world
                .world_pose(id)
                .filter(|_| drawn(world, id) && !on_bone(scene, id));
            let Some(now) = now else {
                self.at_tick.remove(&id);
                continue;
            };
            match self.at_tick.insert(id, now) {
                Some(before) if finite(before) && finite(now) => {
                    from.insert(id, before);
                }
                _ => {}
            }
        }
        (Some(from), cuts)
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
        let tracked = self.tracked.as_ref()?;
        if tracked.scene != scene.id() || self.alpha >= 1.0 {
            return None;
        }
        let before = *self.from.get(&id)?;
        let now = scene.world.world_pose(id)?;
        // Moved since the tick (an edit between ticks): drawn where it is.
        (self.at_tick.get(&id) == Some(&now)).then_some((before, now))
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

fn drawn(world: &World, id: u32) -> bool {
    world.mesh(id).is_some()
        || world.camera(id).is_some()
        || world.has_collider(id)
        || world.has_character_controller(id)
}

/// The one guard left of the old distance rule: a lost pose is never blended through.
fn finite(pose: Pose) -> bool {
    pose.pos.is_finite() && pose.rot.is_finite()
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
#[path = "cut_tests.rs"]
mod cut_tests;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
