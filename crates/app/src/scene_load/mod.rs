//! Unity's `SceneManager.LoadScene`: `Scene.Load` only queues; this tail-of-tick system drains
//! one load a tick, in request order. A `"Single"` one tears the World down (`OnDisable`, then
//! `OnDestroy`; `DontDestroyOnLoad` survivors keep their ids); an `"Additive"` one joins it.
mod additive;
mod background;
mod boot;

pub use background::{
    prepare_counted, prepare_in_background, Inline, Loading, PrepareSteps, SceneLoader, ABANDONED,
};
#[doc(hidden)]
pub mod project_fixture;
pub use boot::{load_boot_scene, ProjectSource};

use rusty_edit::load::prepare::{PreparedScene, SceneSidecars};
use rusty_physics::PhysicsWorld;
use rusty_scene::runtime::loads::{Due, LoadMode, LoadStatus};
use rusty_scene::serialize::RehydratedScene;

use super::game::GameWorld;
use super::probe::{timed, LoadProbe};
use super::resources::Resources;
use super::world::World;

/// The scene file editor Play started in, put back on Stop after a play-mode `Scene.Load`.
pub struct PlayOrigin {
    path: Option<String>,
}

impl GameWorld {
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

    pub(super) fn restore_play_origin(&mut self) {
        let Some(origin) = self.resources.play_origin.take() else {
            return;
        };
        *self.resources.script_manager.scene_path_cell().borrow_mut() = origin.path;
    }
}

pub(super) fn apply_scene_load(world: &mut World, res: &mut Resources) {
    let Some(due) = world.scene.borrow_mut().loads.take_due() else {
        return;
    };
    let mut probe = res.load_probes.scene.take();
    let loaded = match due {
        Due::Sync(path, mode) => match prepare(path, &mut probe) {
            Ok(prepared) => Some(activate_in(world, res, prepared, mode, &mut probe)),
            Err(err) => {
                res.console.borrow_mut().error(format!("Scene.Load: {err}"));
                None
            }
        },
        Due::Async { op, path, mode } => activate_ready(world, res, op, &path, mode, &mut probe),
    };
    if let Some(probe) = probe.as_mut() {
        probe.finish(loaded.as_deref(), &mut res.console.borrow_mut());
    }
    res.load_probes.scene = probe;
}

fn activate_ready(
    world: &mut World,
    res: &mut Resources,
    op: u32,
    path: &str,
    mode: LoadMode,
    probe: &mut Option<Box<dyn LoadProbe>>,
) -> Option<String> {
    let held = res.loading.ready.take().filter(|(at, _)| *at == op);
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
    let loaded = activate_in(world, res, prepared, mode, probe);
    world.scene.borrow_mut().loads.finish(op, LoadStatus::Done);
    Some(loaded)
}

/// The load's first stage, from the files alone. Fails before the current scene is touched.
fn prepare(path: String, probe: &mut Option<Box<dyn LoadProbe>>) -> Result<PreparedScene, String> {
    let data = timed(probe, "read", || rusty_scene::read_scene_file(&path))?;
    let document = timed(probe, "meshes", || RehydratedScene::new(data));
    let sidecars = timed(probe, "sidecars", || SceneSidecars::read(&path));
    Ok(PreparedScene::new(path, document, sidecars))
}

fn activate_in(
    world: &mut World,
    res: &mut Resources,
    prepared: PreparedScene,
    mode: LoadMode,
    probe: &mut Option<Box<dyn LoadProbe>>,
) -> String {
    match mode {
        LoadMode::Single => activate(world, res, prepared, probe),
        LoadMode::Additive => additive::activate(world, res, prepared, probe),
    }
}

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
        res.script_manager.forget_queries();
        stop_voices(world, res, &outgoing);
    });
    crate::systems::origin::to_authored(world, res);
    let swapped = timed(probe, "swap", || {
        rusty_edit::load::swap_in_prepared(&mut (world.scene.borrow_mut()), prepared, &survivors)
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
        // Restored from the new scene's saved navmesh, or prepare's bake, or baked once:
        // the next tick only syncs.
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

fn stop_voices(world: &World, res: &Resources, ids: &[u32]) {
    let scene = world.scene.borrow();
    let at = |&id: &u32| (id, scene.world.world_position(id).to_array());
    let tick = res.play_frame();
    let mut audio = res.audio.borrow_mut();
    audio.stop_sources(ids.iter().map(at), tick);
    audio.forget_motion(); // the camera may land anywhere: no doppler chirp
}

#[cfg(test)]
#[path = "fixture.rs"]
mod fixture;
#[cfg(test)]
#[path = "additive_tests.rs"]
mod scene_load_additive_tests;
#[cfg(test)]
#[path = "api_tests.rs"]
mod scene_load_api_tests;
#[cfg(test)]
#[path = "async_tests.rs"]
mod scene_load_async_tests;
#[cfg(test)]
#[path = "navmesh_tests.rs"]
mod scene_load_navmesh_tests;
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
#[path = "progress_tests.rs"]
mod scene_load_progress_tests;
#[cfg(test)]
#[path = "tests.rs"]
mod scene_load_tests;
