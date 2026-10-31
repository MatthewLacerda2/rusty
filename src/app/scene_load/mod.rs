//! src/app/scene_load/mod.rs — the scene-load phase (#432), Unity's `SceneManager.LoadScene`.
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

mod boot;

#[cfg(test)]
pub(crate) use boot::project_tests;
pub use boot::{load_boot_scene, ProjectSource};

use crate::physics::PhysicsWorld;

use super::game::GameWorld;
use super::resources::Resources;
use super::world::World;

/// Where an editor Play session started: the scene file, so Stop can put it back
/// after a play-mode `Scene.Load` swapped it. The scene's identity comes back with
/// the edit snapshot (`SceneSnapshot`, #1649).
pub struct PlayOrigin {
    path: Option<String>,
}

impl GameWorld {
    /// Remember the scene file Play starts in (taken with the snapshot).
    pub(super) fn capture_play_origin(&mut self) {
        self.resources.play_origin = Some(PlayOrigin {
            path: self
                .resources
                .script_manager
                .scene_path_cell()
                .borrow()
                .clone(),
        });
    }

    /// On Stop, after the snapshot restore: hand back the scene file Play started
    /// in. The snapshot gave back the scene's identity, so a bake waiting for Stop
    /// still applies (#1374), and leaving Play dropped any load or destroy still
    /// queued and every `DontDestroyOnLoad` mark (`Scene::set_playing`).
    pub(super) fn restore_play_origin(&mut self) {
        let Some(origin) = self.resources.play_origin.take() else {
            return;
        };
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
    let at = |&id: &u32| (id, scene.world.world_position(id).to_array());
    let tick = res.play_frame();
    res.audio
        .borrow_mut()
        .stop_sources(ids.iter().map(at), tick);
}

#[cfg(test)]
#[path = "fixture.rs"]
mod fixture;
#[cfg(test)]
#[path = "api_tests.rs"]
mod scene_load_api_tests;
#[cfg(test)]
#[path = "play_state_tests.rs"]
mod scene_load_play_state_tests;
#[cfg(test)]
#[path = "tests.rs"]
mod scene_load_tests;
