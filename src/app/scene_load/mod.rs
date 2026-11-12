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
//! The load runs in two stages (#2123): [`prepare`] reads everything it can from the
//! files alone into a `PreparedScene` (`scene::runtime::prepare`), then [`activate`]
//! swaps it in. Each step runs under the installed `LoadProbe`, which times it. A
//! `Scene.Load` runs both here, at once; a `Scene.LoadAsync` operation was prepared
//! by the host's loader (`background`) and only activates here, on the first tick
//! tail that finds it ready with its activation allowed.
//!
//! Editor Play: Stop restores the edit snapshot taken at Play — the scene Play was
//! pressed in, not the one play ended in — together with its scene-file path.

mod background;
mod boot;

pub use background::{prepare_in_background, Inline, SceneLoader};
#[cfg(test)]
pub(crate) use boot::project_tests;
pub use boot::{load_boot_scene, ProjectSource};

use crate::physics::PhysicsWorld;
use crate::scene::runtime::loads::{Due, LoadStatus};
use crate::scene::runtime::prepare::{PreparedScene, SceneSidecars};
use crate::scene::serialize::RehydratedScene;

use super::game::GameWorld;
use super::probe::{timed, LoadProbe};
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
/// Prepared from its files, then activated, each step under the load probe.
pub(super) fn apply_scene_load(world: &mut World, res: &mut Resources) {
    let Some(due) = world.scene.borrow_mut().loads.take_due() else {
        return;
    };
    let mut probe = res.load_probe.take();
    let loaded = match due {
        Due::Sync(path) => match prepare(path, &mut probe) {
            Ok(prepared) => Some(activate(world, res, prepared, &mut probe)),
            Err(err) => {
                res.console.borrow_mut().error(format!("Scene.Load: {err}"));
                None
            }
        },
        Due::Async { op, path } => activate_ready(world, res, op, &path, &mut probe),
    };
    if let Some(probe) = probe.as_mut() {
        probe.finish(loaded.as_deref(), &mut res.console.borrow_mut());
    }
    res.load_probe = probe;
}

/// Activate operation `op`, whose scene the tick head published ready; it ends done.
fn activate_ready(
    world: &mut World,
    res: &mut Resources,
    op: u32,
    path: &str,
    probe: &mut Option<Box<dyn LoadProbe>>,
) -> Option<String> {
    let held = res.ready_scene.take().filter(|(at, _)| *at == op);
    let Some((_, prepared)) = held else {
        let why = format!("Scene.LoadAsync: {path}: the prepared scene was lost");
        res.console.borrow_mut().error(why);
        world
            .scene
            .borrow_mut()
            .loads
            .finish(op, LoadStatus::Failed);
        return None;
    };
    let loaded = activate(world, res, prepared, probe);
    world.scene.borrow_mut().loads.finish(op, LoadStatus::Done);
    Some(loaded)
}

/// The load's first stage, from the files alone (`scene::runtime::prepare`): read
/// and parse the document, rebuild its meshes, read its sidecars. Fails, before
/// anything of the current scene is touched, on a file that cannot be read or parsed.
fn prepare(path: String, probe: &mut Option<Box<dyn LoadProbe>>) -> Result<PreparedScene, String> {
    let data = timed(probe, "read", || crate::scene::read_scene_file(&path))?;
    let document = timed(probe, "meshes", || RehydratedScene::new(data));
    let sidecars = timed(probe, "sidecars", || SceneSidecars::read(&path));
    Ok(PreparedScene::new(path, document, sidecars))
}

/// The load's second stage, at the tick's tail: tear the outgoing scene down, swap
/// the prepared one in around the survivors, rebuild physics and the navmesh, start
/// its sound sources. Returns the scene's path.
fn activate(
    world: &mut World,
    res: &mut Resources,
    mut prepared: PreparedScene,
    probe: &mut Option<Box<dyn LoadProbe>>,
) -> String {
    let path = prepared.path().to_string();
    let baked = prepared.take_baked_navmesh();
    let (survivors, outgoing) = {
        let scene = world.scene.borrow();
        let survivors = scene.load_survivors();
        let mut outgoing = scene.entity_ids();
        outgoing.retain(|id| !survivors.contains(id));
        outgoing.sort_unstable();
        (survivors, outgoing)
    };

    timed(probe, "teardown", || {
        res.script_manager.unload_entities(&outgoing);
        res.script_manager.forget_queries(); // made against the outgoing scene (#2116)
        stop_voices(world, res, &outgoing);
    });
    let swapped = timed(probe, "swap", || {
        (world.scene.borrow_mut()).swap_in_prepared(prepared, &survivors)
    });
    if let Err(err) = swapped {
        res.console.borrow_mut().error(format!("Scene.Load: {err}"));
    }
    res.event_system
        .borrow_mut()
        .retain_entities(|id| survivors.contains(&id));
    {
        let mut scene = world.scene.borrow_mut();
        let physics = timed(probe, "physics", || PhysicsWorld::from_scene(&scene));
        *res.physics.borrow_mut() = Some(physics);
        // Restored from the new scene's saved navmesh, or the one prepare baked from
        // its files, or baked, once (#1887): the next tick only syncs.
        let mut nav = res.nav.borrow_mut();
        timed(probe, "navmesh", || {
            nav.start_tracked_with(&mut scene, baked.as_deref())
        });
    }
    *res.script_manager.scene_path_cell().borrow_mut() = Some(path.clone());
    timed(probe, "sources", || {
        super::audio::start_sources(world, res, |id| !survivors.contains(&id));
    });
    res.console
        .borrow_mut()
        .info(format!("Loaded scene {path}"));
    path
}

/// Silence the entity voices of the unloaded entities, logging each stop — a sound
/// source leaves with its scene unless it was marked `DontDestroyOnLoad`.
fn stop_voices(world: &World, res: &Resources, ids: &[u32]) {
    let scene = world.scene.borrow();
    let at = |&id: &u32| (id, scene.world.world_position(id).to_array());
    let tick = res.play_frame();
    let mut audio = res.audio.borrow_mut();
    audio.stop_sources(ids.iter().map(at), tick);
    audio.forget_motion(); // the camera may land anywhere: no doppler chirp (#1749)
}

#[cfg(test)]
#[path = "fixture.rs"]
mod fixture;
#[cfg(test)]
#[path = "api_tests.rs"]
mod scene_load_api_tests;
#[cfg(test)]
#[path = "new_tests.rs"]
mod scene_load_new_tests;
#[cfg(test)]
#[path = "play_state_tests.rs"]
mod scene_load_play_state_tests;
#[cfg(test)]
#[path = "probe_tests.rs"]
mod scene_load_probe_tests;
#[cfg(test)]
#[path = "tests.rs"]
mod scene_load_tests;
