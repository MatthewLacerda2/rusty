//! src/app/scene_load.rs — the scene-load phase (#432), Unity's `SceneManager.LoadScene`.
//!
//! Play-mode `Scene.Load(path)` only queues the path on the scene. This system,
//! registered at the tick's tail right after the destroy phase, drains it: the
//! outgoing scene's scripts get `OnDisable` then `OnDestroy` (a scene change is
//! gameplay, unlike Stop), and the World is swapped for the new document with the
//! `DontDestroyOnLoad` survivors carried across under their own ids. Everything else
//! that held an outgoing entity id is cleared with it: script instances, timers and
//! coroutines, UI focus/hover/press state, entity voices. Physics and the navmesh are
//! rebuilt from the new scene, and the new scene's scripts `Awake`/`Start` at the head
//! of the next tick, exactly like spawns. A file that cannot be read or parsed is
//! reported to the console and the current scene keeps running.
//!
//! Editor Play: Stop restores the edit snapshot taken at Play — the scene Play was
//! pressed in, not the one play ended in — together with its scene-file path.

use crate::physics::PhysicsWorld;
use crate::scene::SceneId;

use super::game::GameWorld;
use super::resources::Resources;
use super::world::World;

/// Where an editor Play session started: the scene file and the scene's runtime
/// identity, so Stop can put both back after a play-mode `Scene.Load` swapped them.
pub struct PlayOrigin {
    path: Option<String>,
    scene: SceneId,
}

impl GameWorld {
    /// Remember the scene file and identity Play starts in (taken with the snapshot).
    pub(super) fn capture_play_origin(&mut self) {
        self.resources.play_origin = Some(PlayOrigin {
            path: self
                .resources
                .script_manager
                .scene_path_cell()
                .borrow()
                .clone(),
            scene: self.world.scene.borrow().id(),
        });
    }

    /// On Stop, after the snapshot restore: drop any load still queued and every
    /// `DontDestroyOnLoad` mark, and — if play loaded another scene — hand back the
    /// original scene file and a fresh identity (the restored World is a different
    /// scene from the one the renderer last cached).
    pub(super) fn restore_play_origin(&mut self) {
        let mut scene = self.world.scene.borrow_mut();
        scene.pending_load = None;
        scene.persistent.clear();
        let Some(origin) = self.resources.play_origin.take() else {
            return;
        };
        if scene.id() != origin.scene {
            scene.renew_id();
        }
        *self.resources.script_manager.scene_path_cell().borrow_mut() = origin.path;
    }
}

/// The scene-load phase: swap in the scene a `Scene.Load` queued this tick, if any.
pub(super) fn apply_scene_load(world: &mut World, res: &mut Resources) {
    let Some(path) = world.scene.borrow_mut().take_pending_load() else {
        return;
    };
    let data = match crate::scene::read_scene_file(&path) {
        Ok(data) => data,
        Err(err) => {
            res.console.borrow_mut().error(format!("Scene.Load: {err}"));
            return;
        }
    };
    let (survivors, outgoing) = {
        let scene = world.scene.borrow();
        let survivors = scene.load_survivors();
        let mut outgoing = scene.entity_ids();
        outgoing.retain(|id| !survivors.contains(id));
        outgoing.sort_unstable();
        (survivors, outgoing)
    };

    res.script_manager.unload_entities(&outgoing);
    stop_voices(world, res, &outgoing);
    let swapped = world.scene.borrow_mut().swap_in(data, &path, &survivors);
    if let Err(err) = swapped {
        res.console.borrow_mut().error(format!("Scene.Load: {err}"));
    }
    res.event_system
        .borrow_mut()
        .retain_entities(|id| survivors.contains(&id));
    {
        let scene = world.scene.borrow();
        *res.physics.borrow_mut() = Some(PhysicsWorld::from_scene(&scene));
        res.nav.borrow_mut().bake(&scene);
    }
    *res.script_manager.scene_path_cell().borrow_mut() = Some(path.clone());
    super::audio::start_sources(world, res, |id| !survivors.contains(&id));
    res.console
        .borrow_mut()
        .info(format!("Loaded scene {path}"));
}

/// Silence the entity voices of the unloaded entities, logging each stop — a sound
/// source leaves with its scene unless it was marked `DontDestroyOnLoad`.
fn stop_voices(world: &World, res: &Resources, ids: &[u32]) {
    let scene = world.scene.borrow();
    let tick = res.play_frame();
    let mut audio = res.audio.borrow_mut();
    let playing: Vec<u32> = ids
        .iter()
        .copied()
        .filter(|&id| audio.is_source_playing(id))
        .collect();
    for id in playing {
        let p = scene
            .world
            .transform(id)
            .map(|t| t.position)
            .unwrap_or_default();
        audio.stop_source(id, [p.x, p.y, p.z], tick, true);
    }
}

#[cfg(test)]
#[path = "scene_load_fixture.rs"]
mod fixture;
#[cfg(test)]
#[path = "scene_load_api_tests.rs"]
mod scene_load_api_tests;
#[cfg(test)]
#[path = "scene_load_tests.rs"]
mod scene_load_tests;
