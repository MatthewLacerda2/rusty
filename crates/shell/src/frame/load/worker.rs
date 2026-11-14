//! crates/shell/src/frame/load/worker.rs — the windowed shell's `SceneLoader` (#2123): a
//! `Scene.LoadAsync` operation is prepared on a thread of its own
//! (`app::prepare_counted`) while the current scene keeps ticking and drawing, then,
//! in a loader that [`warms`](Threaded::warming), its GPU resources are made a slice
//! per frame (`render::warm`, fed by `super::warm`) before it reads finished.
//!
//! The world asks it only at a tick's head (`app::scene_load::background`), so when
//! the thread and the warm-up finish decides only which tick the operation reads
//! ready on, and the progress a script reads meanwhile: inputs, which a demo
//! records. One job at a time; starting another, cancelling or dropping the loader
//! abandons the one before: its thread stops at the next step of the prepare
//! (`app::scene_load::ABANDONED`) and its warm-up with it, so a script that calls
//! `LoadAsync` again before the last one finished, or a Stop mid-load, leaves no
//! full preparation running beside the ticking thread.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread::JoinHandle;

use rusty_app::{prepare_counted, PrepareSteps, SceneLoader};
use rusty_render::warm::Warming;
use rusty_scene::runtime::prepare::PreparedScene;
use rusty_scene::serialize::SceneData;

type Prepared = Result<PreparedScene, String>;

/// What the thread sends: the prepared scene, and its warm-up's plan when the loader
/// warms (planned there too, so the tick thread never walks the document).
type Answer = (Prepared, Option<Warming>);

/// One operation's job: its thread's channel, the result once it came, the flag
/// that abandons it (set when the job is dropped), the steps its thread passed, and
/// the GPU warm-up of its scene once it is prepared.
struct Job {
    op: u32,
    rx: Receiver<Answer>,
    done: Option<Prepared>,
    abandoned: Arc<AtomicBool>,
    steps: Arc<PrepareSteps>,
    warm: Option<Warming>,
}

impl Drop for Job {
    fn drop(&mut self) {
        self.abandoned.store(true, Ordering::Relaxed);
    }
}

impl Job {
    /// Take the thread's answer if it came; whether the job is prepared (or failed).
    fn poll(&mut self) -> bool {
        if self.done.is_none() {
            self.done = match self.rx.try_recv() {
                Ok((prepared, warm)) => {
                    self.warm = warm;
                    Some(prepared)
                }
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => Some(Err(STOPPED.into())),
            };
        }
        self.done.is_some()
    }

    /// The prepared scene and its warm-up; `None` before the thread answered, for a
    /// job that failed, or in a loader that does not warm.
    fn warming(&mut self) -> Option<(&SceneData, &mut Warming)> {
        let Some(Ok(prepared)) = &self.done else {
            return None;
        };
        Some((prepared.data(), self.warm.as_mut()?))
    }
}

/// Prepare `path` on a thread of its own, which checks `abandoned` between steps
/// and counts them in `steps`, and plans its warm-up when `warms`.
fn spawn(
    path: String,
    abandoned: Arc<AtomicBool>,
    steps: Arc<PrepareSteps>,
    warms: bool,
) -> (Receiver<Answer>, std::io::Result<JoinHandle<()>>) {
    let (tx, rx) = channel();
    let thread = std::thread::Builder::new()
        .name("scene-load".into())
        .spawn(move || {
            let prepared = prepare_counted(&path, &abandoned, &steps);
            let plan = |p: &PreparedScene| Warming::plan(p.data());
            let warm = prepared.as_ref().ok().filter(|_| warms).map(plan);
            drop(tx.send((prepared, warm)));
        });
    (rx, thread)
}

/// Prepares each operation on a thread of its own; [`Threaded::warming`] also holds
/// it until its GPU resources are made.
#[derive(Default)]
pub struct Threaded {
    job: Option<Job>,
    warms: bool,
}

impl Threaded {
    /// A loader that holds each prepared operation back until the renderer has
    /// warmed it ([`Self::to_warm`]): the windowed shell's, which draws.
    pub fn warming() -> Self {
        Self {
            job: None,
            warms: true,
        }
    }

    /// The pending job's prepared document and what is left to warm of it, when
    /// this loader warms and the job is prepared and not yet warm.
    pub fn to_warm(&mut self) -> Option<(&SceneData, &mut Warming)> {
        let job = self.job.as_mut().filter(|_| self.warms)?;
        job.poll();
        job.warming().filter(|(_, warm)| !warm.is_done())
    }
}

impl SceneLoader for Threaded {
    fn start(&mut self, op: u32, path: String) {
        self.job = None; // the one before is abandoned first
        let abandoned = Arc::new(AtomicBool::new(false));
        let steps = Arc::new(PrepareSteps::default());
        let (rx, spawned) = spawn(path, abandoned.clone(), steps.clone(), self.warms);
        // No thread to be had: the job fails, and the current scene keeps running.
        let done = spawned
            .err()
            .map(|e| Err(format!("no load thread could start ({e})")));
        self.job = Some(Job {
            op,
            rx,
            done,
            abandoned,
            steps,
            warm: None,
        });
    }

    fn finished(&mut self, op: u32) -> bool {
        let warms = self.warms;
        let Some(job) = self.job.as_mut().filter(|job| job.op == op) else {
            return false;
        };
        if !job.poll() {
            return false;
        }
        match (warms, job.warming()) {
            (true, Some((_, warm))) => warm.is_done(),
            _ => true, // a failed job is finished: it has nothing to warm
        }
    }

    fn take(&mut self, op: u32) -> Prepared {
        match self.job.take_if(|job| job.op == op) {
            Some(mut job) => (job.done.take()).unwrap_or_else(|| {
                job.rx
                    .recv()
                    .map_or_else(|_| Err(STOPPED.into()), |(p, _)| p)
            }),
            None => Err(format!("no load operation {op} was started")),
        }
    }

    fn cancel(&mut self) {
        self.job = None;
    }

    /// The prepare's steps, then (in a loader that warms) the warm-up's share:
    /// half each.
    fn progress(&mut self, op: u32) -> f32 {
        let warms = self.warms;
        let Some(job) = self.job.as_mut().filter(|job| job.op == op) else {
            return 0.0;
        };
        let prepared = job.steps.fraction();
        match (warms, job.warm.as_ref()) {
            (false, _) => prepared,
            (true, None) => prepared * 0.5,
            (true, Some(warm)) => 0.5 + warm.progress() * 0.5,
        }
    }
}

/// What a job whose thread ended without an answer (a panic) reports.
const STOPPED: &str = "the load thread stopped before the scene was prepared";

#[cfg(test)]
#[path = "worker_tests.rs"]
mod tests;
