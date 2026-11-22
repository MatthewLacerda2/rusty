//! Which tick an operation becomes ready on is an **input**, like `InputFrame`: the tap
//! may change what the loader reports.
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use rusty_edit::load::prepare::{staged, PreparedScene, SceneSidecars};
use rusty_navigation::NavigationGraph;
use rusty_scene::serialize::RehydratedScene;

use super::super::game::GameWorld;

/// `finished` is asked every tick head, so never waits; `take` may; `start` drops any
/// earlier job.
pub trait SceneLoader: std::any::Any {
    fn start(&mut self, op: u32, path: String);
    fn finished(&mut self, op: u32) -> bool;
    fn take(&mut self, op: u32) -> Result<PreparedScene, String>;
    fn cancel(&mut self);
    /// From 0 to 1, a script's `progress` (x 0.9) before ready; 0 when unknown.
    fn progress(&mut self, _op: u32) -> f32 {
        0.0
    }
    /// Whether `take` prepares on the ticking thread.
    fn prepares_inline(&self) -> bool {
        false
    }
}

pub struct Loading {
    pub(crate) loader: Box<dyn SceneLoader>,
    pub(crate) ready: Option<(u32, PreparedScene)>,
}

impl Default for Loading {
    fn default() -> Self {
        Self {
            loader: Box::new(Inline::default()),
            ready: None,
        }
    }
}

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

    fn prepares_inline(&self) -> bool {
        true
    }
}

pub const ABANDONED: &str = "the load was abandoned";

/// Stamps the write log first, so a model written meanwhile is still followed.
pub fn prepare_in_background(path: &str, abandoned: &AtomicBool) -> Result<PreparedScene, String> {
    prepare_counted(path, abandoned, &PrepareSteps::default())
}

#[derive(Debug)]
pub struct PrepareSteps {
    done: AtomicU8,
    total: AtomicU8,
}

impl Default for PrepareSteps {
    fn default() -> Self {
        Self {
            done: AtomicU8::new(0),
            total: AtomicU8::new(4),
        }
    }
}

impl PrepareSteps {
    pub fn fraction(&self) -> f32 {
        let total = self.total.load(Ordering::Relaxed).max(1);
        let done = self.done.load(Ordering::Relaxed).min(total);
        f32::from(done) / f32::from(total)
    }

    pub fn total(&self) -> u8 {
        self.total.load(Ordering::Relaxed)
    }
}

pub fn prepare_counted(
    path: &str,
    abandoned: &AtomicBool,
    steps: &PrepareSteps,
) -> Result<PreparedScene, String> {
    let step = || match abandoned.load(Ordering::Relaxed) {
        true => Err(ABANDONED.to_string()),
        false => {
            steps.done.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    };
    // The conversion gate is per thread: a worker preparing this load takes its own hold.
    let _hold = rusty_asset::convert::PlayHold::take();
    let read_at = rusty_core::fs::written::written_since(u64::MAX).0;
    let data = rusty_scene::read_scene_file(path)?;
    step()?;
    let sidecars = SceneSidecars::read(path);
    if sidecars.has_navmesh() {
        steps.total.store(3, Ordering::Relaxed);
    }
    let document = RehydratedScene::read_since(data, read_at);
    step()?;
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

/// Staging applies the document as activation will again: warnings are left to that one.
fn bake_navmesh(
    document: &RehydratedScene,
    step: impl Fn() -> Result<(), String>,
) -> Result<Vec<u8>, String> {
    let scene = rusty_core::quiet::quietly(|| staged(document.clone()));
    step()?;
    Ok(NavigationGraph::baked_bytes(&scene))
}

impl GameWorld {
    pub fn set_scene_loader(&mut self, loader: Box<dyn SceneLoader>) {
        self.resources.loading.loader = loader;
    }

    pub fn scene_loader_mut<T: SceneLoader>(&mut self) -> Option<&mut T> {
        let loader: &mut dyn std::any::Any = self.resources.loading.loader.as_mut();
        loader.downcast_mut::<T>()
    }

    pub(crate) fn start_loads(&mut self) {
        let job = self.world.scene.borrow_mut().loads.to_start();
        if let Some((op, path)) = job {
            self.resources.loading.loader.start(op, path);
        }
    }

    pub(crate) fn publish_loads(&mut self) {
        self.start_loads();
        let res = &mut self.resources;
        let pending = self.world.scene.borrow().loads.pending_op();
        if res
            .loading
            .ready
            .as_ref()
            .is_some_and(|(op, _)| Some(*op) != pending)
        {
            res.loading.ready = None;
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
            let prepared = self.take_load(op);
            let res = &mut self.resources;
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
