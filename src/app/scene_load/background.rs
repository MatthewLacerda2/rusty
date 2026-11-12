//! src/app/scene_load/background.rs — `Scene.LoadAsync`'s host side (#2123).
//!
//! An operation's scene is prepared by the host's [`SceneLoader`], never by a
//! system: the windowed shell's runs [`prepare_in_background`] on a worker thread
//! (`shell::load_worker`); the default, [`Inline`], runs it at the tick head that
//! asks, so every headless host (the harness, `play`, `game --test`) reads the
//! operation ready from the first tick after the request, every run.
//!
//! Which tick an operation becomes ready on is an **input**, as `InputFrame` is: the
//! world asks the loader only at the tick's head ([`GameWorld::publish_loads`],
//! after the tap's `BeforeInput`), the tap sees what it reported and may change it
//! (`TickTap::loads`: a demo records it, a replay hands back the recorded ticks and
//! waits for its own loader there), and only then does the operation's status move.
//! What the operation loads is the files' alone, so it is the same on every host.

use crate::navigation::NavigationGraph;
use crate::scene::runtime::prepare::{staged, PreparedScene, SceneSidecars};
use crate::scene::serialize::RehydratedScene;

use super::super::game::GameWorld;

/// Prepares `Scene.LoadAsync` operations for a world: installed by its host with
/// [`GameWorld::set_scene_loader`]. One job at a time.
pub trait SceneLoader {
    /// Begin preparing `path` for operation `op`, dropping any job before it.
    fn start(&mut self, op: u32, path: String);
    /// Whether the job for `op` is finished, without waiting for it.
    fn finished(&mut self, op: u32) -> bool;
    /// The job's result, waiting for it while it runs.
    fn take(&mut self, op: u32) -> Result<PreparedScene, String>;
    /// Drop the job, if any: nothing is pending any more.
    fn cancel(&mut self);
}

/// The default loader: prepares at the tick head that takes it, on the ticking thread.
#[derive(Default)]
pub struct Inline {
    job: Option<(u32, String)>,
}

impl SceneLoader for Inline {
    fn start(&mut self, op: u32, path: String) {
        self.job = Some((op, path));
    }

    fn finished(&mut self, op: u32) -> bool {
        self.job.as_ref().is_some_and(|(at, _)| *at == op)
    }

    fn take(&mut self, op: u32) -> Result<PreparedScene, String> {
        match self.job.take_if(|(at, _)| *at == op) {
            Some((_, path)) => prepare_in_background(&path),
            None => Err(format!("no load operation {op} was started")),
        }
    }

    fn cancel(&mut self) {
        self.job = None;
    }
}

/// Everything a `Scene.LoadAsync` prepares from the files alone, on any thread: the
/// document read and parsed, its meshes rebuilt, its sidecars read, and the navmesh
/// baked when none is saved. No conversion runs meanwhile (a load happens in Play,
/// `asset::convert::PlayHold`), and the write-log stamp is taken before anything is
/// read, so a model written while it ran is still followed.
pub fn prepare_in_background(path: &str) -> Result<PreparedScene, String> {
    let _hold = crate::asset::convert::PlayHold::take();
    let read_at = crate::core::fs::written::written_since(u64::MAX).0;
    let data = crate::scene::read_scene_file(path)?;
    let document = RehydratedScene::read_since(data, read_at);
    let sidecars = SceneSidecars::read(path);
    let mut prepared = PreparedScene::new(path.to_string(), document, sidecars);
    if !prepared.has_saved_navmesh() {
        if let Some(bytes) = bake_navmesh(path) {
            prepared.set_baked_navmesh(bytes);
        }
    }
    Ok(prepared)
}

/// Bake `path`'s navmesh from a staged copy of its document: the bytes activation
/// restores when they still match the live scene (`NavigationGraph::start_tracked_with`).
fn bake_navmesh(path: &str) -> Option<Vec<u8>> {
    let data = crate::scene::read_scene_file(path).ok()?;
    let scene = staged(RehydratedScene::new(data));
    Some(NavigationGraph::baked_bytes(&scene))
}

impl GameWorld {
    /// Install the host's [`SceneLoader`] (the default prepares inline).
    pub fn set_scene_loader(&mut self, loader: Box<dyn SceneLoader>) {
        self.resources.scene_loader = loader;
    }

    /// Hand the loader the operation a script opened since it last looked.
    pub(in crate::app) fn start_loads(&mut self) {
        let job = self.world.scene.borrow_mut().loads.to_start();
        if let Some((op, path)) = job {
            self.resources.scene_loader.start(op, path);
        }
    }

    /// The tick head's publish: what the loader finished, as the tap leaves it, moves
    /// the operation to ready (its scene held for the tail) or failed. A load no
    /// longer pending drops its job and any scene held for it.
    pub(in crate::app) fn publish_loads(&mut self) {
        self.start_loads();
        let res = &mut self.resources;
        if !self.world.scene.borrow().loads.has_pending() {
            res.ready_scene = None;
            res.scene_loader.cancel();
        }
        let waiting = self.world.scene.borrow().loads.waiting();
        let live: Vec<u32> = waiting
            .filter(|&op| res.scene_loader.finished(op))
            .into_iter()
            .collect();
        let ready = match res.tap.as_mut() {
            Some(tap) => tap.loads(live),
            None => live,
        };
        for op in ready {
            if self.world.scene.borrow().loads.waiting() != Some(op) {
                continue;
            }
            let prepared = res.scene_loader.take(op);
            let ok = prepared.is_ok();
            match prepared {
                Ok(scene) => res.ready_scene = Some((op, scene)),
                Err(err) => res
                    .console
                    .borrow_mut()
                    .error(format!("Scene.LoadAsync: {err}")),
            }
            self.world.scene.borrow_mut().loads.publish(op, ok);
        }
    }
}
