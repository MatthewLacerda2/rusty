//! src/app/scene_load/background.rs — `Scene.LoadAsync`'s host side (#2123).
//!
//! An operation's scene is prepared by the host's [`SceneLoader`], never by a
//! system: the windowed shell's runs [`prepare_in_background`] on a worker thread
//! (`shell::frame::load::worker`); the default, [`Inline`], runs it at the tick head that
//! asks, so every headless host (the harness, `play`, `game --test`) reads the
//! operation ready from the first tick after the request, every run.
//!
//! Which tick an operation becomes ready on is an **input**, as `InputFrame` is: the
//! world asks the loader only at the tick's head ([`GameWorld::publish_loads`],
//! after the tap's `BeforeInput`), the tap sees what it reported and may change it
//! (`TickTap::loads`: a demo records it, a replay hands back the recorded ticks and
//! waits for its own loader there), and only then does the operation's status move.
//! What the operation loads is the files' alone, so it is the same on every host.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use crate::navigation::NavigationGraph;
use crate::scene::runtime::prepare::{staged, PreparedScene, SceneSidecars};
use crate::scene::serialize::RehydratedScene;

use super::super::game::GameWorld;

/// Prepares `Scene.LoadAsync` operations for a world: installed by its host with
/// [`GameWorld::set_scene_loader`], reached again with [`GameWorld::scene_loader_mut`].
/// One job at a time.
pub trait SceneLoader: std::any::Any {
    /// Begin preparing `path` for operation `op`, dropping any job before it.
    fn start(&mut self, op: u32, path: String);
    /// Whether the job for `op` is finished, without waiting for it.
    fn finished(&mut self, op: u32) -> bool;
    /// The job's result, waiting for it while it runs.
    fn take(&mut self, op: u32) -> Result<PreparedScene, String>;
    /// Drop the job, if any: nothing is pending any more.
    fn cancel(&mut self);
    /// How far the job for `op` is, from 0 to 1, while it is not finished: what a
    /// script reads as `progress` (× 0.9) before the operation is ready. Asked only
    /// at a tick head; a loader that cannot tell answers 0.
    fn progress(&mut self, _op: u32) -> f32 {
        0.0
    }
}

/// The host's loader — inline by default, a worker thread in the windowed shell
/// (`GameWorld::set_scene_loader`) — and the ready operation's scene, held for the
/// tick's tail.
pub struct Loading {
    pub(in crate::app) loader: Box<dyn SceneLoader>,
    pub(in crate::app) ready: Option<(u32, PreparedScene)>,
}

impl Default for Loading {
    fn default() -> Self {
        Self {
            loader: Box::new(Inline::default()),
            ready: None,
        }
    }
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
            Some((_, path)) => prepare_in_background(&path, &AtomicBool::new(false)),
            None => Err(format!("no load operation {op} was started")),
        }
    }

    fn cancel(&mut self) {
        self.job = None;
    }
}

/// What a job whose operation was replaced, cancelled or stopped reports: it
/// stopped at the next step rather than finishing a scene nobody will take.
pub const ABANDONED: &str = "the load was abandoned";

/// Everything a `Scene.LoadAsync` prepares from the files alone, on any thread: the
/// document read and parsed, its meshes rebuilt, its sidecars read, and the navmesh
/// baked when none is saved. No conversion runs meanwhile (a load happens in Play,
/// `asset::convert::PlayHold`), and the write-log stamp is taken before anything is
/// read, so a model written while it ran is still followed. `abandoned` is checked
/// between steps: once set, the job ends at the next one with [`ABANDONED`].
pub fn prepare_in_background(path: &str, abandoned: &AtomicBool) -> Result<PreparedScene, String> {
    prepare_counted(path, abandoned, &AtomicU8::new(0))
}

/// How many steps [`prepare_counted`] counts at most: the read, the meshes, the
/// navmesh bake's staging and the sidecars (one fewer when a navmesh is saved).
pub const PREPARE_STEPS: u8 = 4;

/// [`prepare_in_background`], counting each step done in `steps` (up to
/// [`PREPARE_STEPS`]), which a loader reads as its progress.
pub fn prepare_counted(
    path: &str,
    abandoned: &AtomicBool,
    steps: &AtomicU8,
) -> Result<PreparedScene, String> {
    let step = || match abandoned.load(Ordering::Relaxed) {
        true => Err(ABANDONED.to_string()),
        false => {
            steps.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    };
    let _hold = crate::asset::convert::PlayHold::take();
    let read_at = crate::core::fs::written::written_since(u64::MAX).0;
    let data = crate::scene::read_scene_file(path)?;
    step()?;
    let document = RehydratedScene::read_since(data, read_at);
    step()?;
    let sidecars = SceneSidecars::read(path);
    let baked = match sidecars.has_navmesh() {
        true => None,
        false => Some(bake_navmesh(&document, step)?),
    };
    step()?;
    let mut prepared = PreparedScene::new(path.to_string(), document, sidecars);
    if let Some(bytes) = baked {
        prepared.set_baked_navmesh(bytes);
    }
    Ok(prepared)
}

/// Bake the navmesh of a staged copy of `document`, the one just read: the bytes
/// activation restores when they still match the live scene
/// (`NavigationGraph::start_tracked_with`). Staging applies the document as the
/// activation will again, so its warnings are left to that one (`core::quiet`).
fn bake_navmesh(
    document: &RehydratedScene,
    step: impl Fn() -> Result<(), String>,
) -> Result<Vec<u8>, String> {
    let scene = crate::core::quiet::quietly(|| staged(document.clone()));
    step()?;
    Ok(NavigationGraph::baked_bytes(&scene))
}

impl GameWorld {
    /// Install the host's [`SceneLoader`] (the default prepares inline).
    pub fn set_scene_loader(&mut self, loader: Box<dyn SceneLoader>) {
        self.resources.loading.loader = loader;
    }

    /// The installed loader, if it is a `T`: the host's own, between ticks.
    pub fn scene_loader_mut<T: SceneLoader>(&mut self) -> Option<&mut T> {
        let loader: &mut dyn std::any::Any = self.resources.loading.loader.as_mut();
        loader.downcast_mut::<T>()
    }

    /// Hand the loader the operation a script opened since it last looked.
    pub(in crate::app) fn start_loads(&mut self) {
        let job = self.world.scene.borrow_mut().loads.to_start();
        if let Some((op, path)) = job {
            self.resources.loading.loader.start(op, path);
        }
    }

    /// The tick head's publish: what the loader finished, as the tap leaves it, moves
    /// the operation to ready (its scene held for the tail) or failed. An operation
    /// no longer pending drops its job and any scene held for it.
    pub(in crate::app) fn publish_loads(&mut self) {
        self.start_loads();
        let res = &mut self.resources;
        let pending = self.world.scene.borrow().loads.pending_op();
        if res
            .loading
            .ready
            .as_ref()
            .is_some_and(|(op, _)| Some(*op) != pending)
        {
            res.loading.ready = None; // replaced, or Play stopped
        }
        if pending.is_none() {
            res.loading.loader.cancel();
        }
        let waiting = self.world.scene.borrow().loads.waiting();
        let live: Vec<u32> = waiting
            .filter(|&op| res.loading.loader.finished(op))
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
            let prepared = res.loading.loader.take(op);
            let ok = prepared.is_ok();
            match prepared {
                Ok(scene) => res.loading.ready = Some((op, scene)),
                Err(err) => res
                    .console
                    .borrow_mut()
                    .error(format!("Scene.LoadAsync: {err}")),
            }
            self.world.scene.borrow_mut().loads.publish(op, ok);
        }
        self.publish_progress();
    }

    /// The tick head's progress: the still-waiting operation's, in whole percent,
    /// set when it moved, as the tap leaves it.
    fn publish_progress(&mut self) {
        let Some(op) = self.world.scene.borrow().loads.waiting() else {
            return;
        };
        let res = &mut self.resources;
        let percent = (res.loading.loader.progress(op).clamp(0.0, 1.0) * 100.0) as u8;
        let moved = (percent != self.world.scene.borrow().loads.loaded()).then_some(percent);
        let moved = match res.tap.as_mut() {
            Some(tap) => tap.load_progress(moved),
            None => moved,
        };
        if let Some(percent) = moved {
            self.world.scene.borrow_mut().loads.set_loaded(op, percent);
        }
    }
}
