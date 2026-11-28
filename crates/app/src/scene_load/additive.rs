//! `Scene.Load(path, "Additive")` at the tick's tail: the scene joins the World beside what is
//! loaded. Nothing loaded is torn down, physics and the navmesh pick the new entities up through
//! the change log as a spawn's, and the new scripts `Awake`/`Start` next tick.
use std::collections::BTreeSet;

use rusty_edit::load::additive::join_prepared;
use rusty_edit::load::prepare::PreparedScene;

use super::super::probe::{timed, LoadProbe};
use super::super::resources::Resources;
use super::super::world::World;

pub(super) fn activate(
    world: &mut World,
    res: &mut Resources,
    prepared: PreparedScene,
    probe: &mut Option<Box<dyn LoadProbe>>,
) -> String {
    let path = prepared.path().to_string();
    let joined = timed(probe, "swap", || {
        join_prepared(&mut world.scene.borrow_mut(), prepared)
    });
    world.scene.borrow_mut().join_shifted(&joined.roots);
    let base = res.script_manager.scene_path_cell().borrow().clone();
    world
        .scene
        .borrow_mut()
        .join_loaded(base, path.clone(), joined.settings, &joined.roots);
    let mut console = res.console.borrow_mut();
    if joined.disagrees {
        console.warn(format!(
            "Scene.Load: {path}: its layers or collision matrix differ from the scene loaded \
             \"Single\", whose are kept"
        ));
    }
    if !joined.skipped.is_empty() {
        console.warn(format!(
            "Scene.Load: {path}: an additive scene's bakes are not applied yet, so it has no {} \
             (bake them into the scene loaded \"Single\")",
            joined.skipped.join(", ")
        ));
    }
    drop(console);
    let ids: BTreeSet<u32> = joined.ids.into_iter().collect();
    timed(probe, "sources", || {
        super::super::audio::start_sources(world, res, |id| ids.contains(&id));
    });
    res.console
        .borrow_mut()
        .info(format!("Loaded scene {path} (additive)"));
    path
}
