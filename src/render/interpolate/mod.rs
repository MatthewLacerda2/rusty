//! src/render/interpolate/ — what a frame draws between two fixed ticks (#1877).
//!
//! The sim runs on the fixed 60 Hz tick (`shell::frame::ticks`); a display at 144 Hz
//! draws two or three frames per tick. Drawn at the tick's poses, motion judders, so
//! every moving entity is drawn **blended between its last two ticks** by how far the
//! frame is into the next one (Unity's `RigidbodyInterpolation.Interpolate`, for every
//! mesh and the main camera). The drawn pose lags the sim by at most one tick.
//!
//! [`DrawPoses`] is the renderer's, never the sim's: the shell [`record`]s it after
//! each tick and sets the blend, the retained scene draws a mesh entity at
//! [`DrawPoses::world`] and every camera entity through [`DrawPoses::pose_camera`]. No
//! sim module reads it, and nothing is written back into the World.
//!
//! **Which entities blend.** Every entity with a mesh or a camera whose world pose
//! the last tick changed (the World's `WorldPose` change log, which a parent's move
//! stamps on its descendants too). An entity with no pose at the tick before (just
//! spawned, the first tick after a load, Play or Stop) is drawn where it is.
//!
//! **A jump is not motion.** A step of more than [`JUMP_DISTANCE`] (2 m in one tick,
//! 120 m/s) or a turn of more than [`JUMP_TURN`] (90° in one tick) is a teleport (a
//! respawn, a script setting the Transform): the entity is drawn at its new pose, never
//! smeared along the way. Anything that fast moves further than the eye follows in a
//! tick, so drawing it at the tick loses nothing.
//!
//! **Every camera entity blends in the renderer, in one place** (`Renderer::render`
//! through [`DrawPoses::pose_camera`]): the main camera, a stacked viewmodel camera
//! and a render-texture camera alike, so a gun parented to a stacked camera's head
//! stays fixed on screen between ticks. The frontends hand `render` the tick's main
//! view, as the headless harness does.
//!
//! **What hangs from a bone is not blended**: an entity with a bone at or above it
//! (a gun on `hand_r`, a camera on the head bone) is drawn at the tick's pose, the
//! one the skinned palette is built from, so it never drifts off the hand that holds
//! it. Its skinned owner's root still blends, and its animation steps at 60 Hz.
//!
//! **What else is not blended** (drawn at the tick's pose): particles, ribbons,
//! decals, lights, and the free camera `Camera.*` drives when no camera entity is
//! main.
//!
//! [`record`]: DrawPoses::record

use std::collections::{BTreeMap, BTreeSet};

use glam::{Mat4, Quat};

use crate::components::transform::Pose;
use crate::ecs::{Changes, Cursor, WorldPose};
use crate::scene::{Camera, Scene, SceneId};

/// A step longer than this in one tick is a teleport, not motion (metres).
pub const JUMP_DISTANCE: f32 = 2.0;
/// A turn larger than this in one tick is a teleport, not motion (radians).
pub const JUMP_TURN: f32 = std::f32::consts::FRAC_PI_2;

/// The poses of the last two ticks of the scene the sim runs. See the module docs.
#[derive(Default)]
pub struct DrawPoses {
    /// The scene recorded, and the cursor on its `WorldPose` log.
    tracked: Option<(SceneId, Cursor<WorldPose>)>,
    /// Every drawn entity's world pose at the last tick.
    at_tick: BTreeMap<u32, Pose>,
    /// The entities the last tick moved (not jumped): their pose the tick before.
    from: BTreeMap<u32, Pose>,
    /// How far the frame is into the next tick, `0..=1`.
    alpha: f32,
    /// What the last extract of that scene drew blended, so one that stops (or a
    /// [`Self::clear`]) is drawn at its pose again.
    drawn: Option<(SceneId, BTreeSet<u32>)>,
}

impl DrawPoses {
    /// After one fixed tick of `scene`: what moved in it, from where.
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
                for id in drawn.chain(world.ids_with_camera()) {
                    if let Some(pose) = world.world_pose(id) {
                        self.at_tick.insert(id, pose);
                    }
                }
            }
            Changes::Ids(ids) => {
                for id in ids {
                    let drawn = world.mesh(id).is_some() || world.camera(id).is_some();
                    let now = world
                        .world_pose(id)
                        .filter(|_| drawn && !on_bone(scene, id));
                    let Some(now) = now else {
                        // Gone, nothing to draw, or posed by a bone (the module docs).
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

    /// How far this frame is into the next tick: `0` draws the tick before, `1` the
    /// last tick.
    pub fn set_alpha(&mut self, alpha: f32) {
        self.alpha = alpha.clamp(0.0, 1.0);
    }

    /// Draw everything at its pose from now on: the sim is not running on the tick
    /// (edit mode, paused, a Play transition, a demo). The next [`Self::record`]
    /// starts afresh. What was drawn blended is still re-posed once.
    pub fn clear(&mut self) {
        self.tracked = None;
        self.at_tick.clear();
        self.from.clear();
        self.alpha = 1.0;
    }

    /// The two poses `id` of `scene` is drawn between this frame (the tick before,
    /// and its live pose), or `None` when it is drawn as it is. A live pose that
    /// jumped from the tick before (a teleport made between ticks: the console, MCP,
    /// a gizmo drag in Play) is drawn where it landed too.
    fn blends(&self, scene: &Scene, id: u32) -> Option<(Pose, Pose)> {
        let (tracked, _) = self.tracked.as_ref()?;
        if *tracked != scene.id() || self.alpha >= 1.0 {
            return None;
        }
        let before = *self.from.get(&id)?;
        let now = scene.world.world_pose(id)?; // only for what blends
        (!jumped(before, now)).then_some((before, now))
    }

    /// Entity `id`'s world matrix as this frame draws it: blended between its last
    /// two ticks while it moves, its live matrix otherwise.
    pub fn world(&self, scene: &Scene, id: u32) -> Mat4 {
        let matrix = scene.world.local_to_world(id);
        match self.blends(scene, id) {
            Some((before, now)) => blend(before, now, self.alpha) * rigid(now).inverse() * matrix,
            None => matrix,
        }
    }

    /// The mesh entities of `scene` to re-pose this frame though the log names none:
    /// every one drawn blended now or at the last extract.
    /// Another scene's extract (a preview, a render texture's) touches nothing.
    pub(crate) fn repose(&mut self, scene: &Scene) -> BTreeSet<u32> {
        let blended = self
            .from
            .keys()
            .filter(|&&id| self.blends(scene, id).is_some());
        let now: BTreeSet<u32> = blended.copied().collect();
        let mut ids = BTreeSet::new();
        match self.drawn.take() {
            Some((drawn, last)) if drawn == scene.id() => ids = last,
            other => self.drawn = other, // another scene's extract: left for its own
        }
        ids.extend(now.iter().copied());
        if !now.is_empty() {
            self.drawn = Some((scene.id(), now));
        }
        ids
    }

    /// Put `camera`, camera entity `id`'s view, where this frame draws that entity:
    /// blended between its last two ticks while it moves, left as it is otherwise.
    /// Every camera entity the renderer draws from goes through here.
    pub fn pose_camera(&self, camera: &mut Camera, scene: &Scene, id: u32) {
        if self.blends(scene, id).is_some() {
            crate::scene::camera::pose_at(camera, self.world(scene, id));
        }
    }
}

/// Whether `id` hangs from a bone (itself one, or under one): the skinned owner
/// above lists a bone at or below that owner on `id`'s chain. Its pose is the
/// animation's at the tick, the one the owner's palette skins by.
fn on_bone(scene: &Scene, id: u32) -> bool {
    let chain: Vec<u32> = scene.world.self_and_ancestors(id).collect();
    chain.iter().enumerate().skip(1).any(|(above, &owner)| {
        let mesh = scene.world.mesh(owner);
        let bones = mesh.as_ref().map(|m| &m.skeleton.bones);
        bones.is_some_and(|b| chain[..above].iter().any(|&c| b.contains(&Some(c))))
    })
}

/// Whether going from `before` to `now` in one tick is a teleport.
fn jumped(before: Pose, now: Pose) -> bool {
    let far = before.pos.distance(now.pos) > JUMP_DISTANCE;
    let unknown = !before.pos.is_finite() || !now.pos.is_finite();
    far || unknown || before.rot.angle_between(now.rot) > JUMP_TURN
}

/// The rigid frame (rotation, then position) of `pose`, without its scale.
fn rigid(pose: Pose) -> Mat4 {
    Mat4::from_rotation_translation(pose.rot, pose.pos)
}

/// The rigid frame `alpha` of the way from `before` to `now`: position linearly,
/// rotation along the shorter arc. The drawn matrix is the live one carried from
/// `now`'s frame to this one, so its scale (and any skew a parent gives it) is the
/// live matrix's, untouched: only position and rotation blend.
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
