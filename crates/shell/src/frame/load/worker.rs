use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread::JoinHandle;

use rusty_app::{prepare_counted, PrepareSteps, SceneLoader};
use rusty_edit::load::prepare::PreparedScene;
use rusty_render::warm::Warming;
use rusty_scene::serialize::SceneData;

type Prepared = Result<PreparedScene, String>;

type Answer = (Prepared, Option<Warming>);

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

    fn warming(&mut self) -> Option<(&SceneData, &mut Warming)> {
        let Some(Ok(prepared)) = &self.done else {
            return None;
        };
        Some((prepared.data(), self.warm.as_mut()?))
    }
}

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
            let prepare = || prepare_counted(&path, &abandoned, &steps);
            let prepared = rusty_asset::cook::par::beside_play(prepare);
            let plan = |p: &PreparedScene| Warming::plan(p.data());
            let warm = prepared.as_ref().ok().filter(|_| warms).map(plan);
            drop(tx.send((prepared, warm)));
        });
    (rx, thread)
}

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

    pub fn to_warm(&mut self) -> Option<(&SceneData, &mut Warming)> {
        let job = self.job.as_mut().filter(|_| self.warms)?;
        job.poll();
        job.warming().filter(|(_, warm)| !warm.is_done())
    }
}

impl SceneLoader for Threaded {
    fn start(&mut self, op: u32, path: String) {
        // Abandons the job before first: its thread stops at its next prepare step.
        self.job = None;
        let abandoned = Arc::new(AtomicBool::new(false));
        let steps = Arc::new(PrepareSteps::default());
        let (rx, spawned) = spawn(path, abandoned.clone(), steps.clone(), self.warms);
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
            // A failed job is finished: it has nothing to warm.
            _ => true,
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
