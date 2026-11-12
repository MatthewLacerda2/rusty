//! src/shell/frame/load_worker.rs — the windowed shell's `SceneLoader` (#2123): a
//! `Scene.LoadAsync` operation is prepared on a thread of its own
//! (`app::prepare_in_background`) while the current scene keeps ticking and drawing.
//!
//! The world asks it only at a tick's head (`app::scene_load::background`), so when
//! the thread finishes decides only which tick the operation reads ready on: an
//! input, which a demo records. One job at a time; starting another, or cancelling,
//! drops the one before, whose thread finishes into a channel nobody reads.

use std::sync::mpsc::{channel, Receiver, TryRecvError};

use crate::app::{prepare_in_background, SceneLoader};
use crate::scene::runtime::prepare::PreparedScene;

type Prepared = Result<PreparedScene, String>;

/// One operation's job: its thread's channel, and the result once it came.
struct Job {
    op: u32,
    rx: Receiver<Prepared>,
    done: Option<Prepared>,
}

/// Prepares each operation on a thread of its own.
#[derive(Default)]
pub struct Threaded {
    job: Option<Job>,
}

impl SceneLoader for Threaded {
    fn start(&mut self, op: u32, path: String) {
        let (tx, rx) = channel();
        let spawned = std::thread::Builder::new()
            .name("scene-load".into())
            .spawn(move || drop(tx.send(prepare_in_background(&path))));
        // No thread to be had: the job fails, and the current scene keeps running.
        let done = spawned
            .err()
            .map(|e| Err(format!("no load thread could start ({e})")));
        self.job = Some(Job { op, rx, done });
    }

    fn finished(&mut self, op: u32) -> bool {
        let Some(job) = self.job.as_mut().filter(|job| job.op == op) else {
            return false;
        };
        if job.done.is_none() {
            job.done = match job.rx.try_recv() {
                Ok(prepared) => Some(prepared),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => Some(Err(STOPPED.into())),
            };
        }
        job.done.is_some()
    }

    fn take(&mut self, op: u32) -> Prepared {
        match self.job.take_if(|job| job.op == op) {
            Some(Job { rx, done, .. }) => {
                done.unwrap_or_else(|| rx.recv().unwrap_or_else(|_| Err(STOPPED.into())))
            }
            None => Err(format!("no load operation {op} was started")),
        }
    }

    fn cancel(&mut self) {
        self.job = None;
    }
}

/// What a job whose thread ended without an answer (a panic) reports.
const STOPPED: &str = "the load thread stopped before the scene was prepared";

#[cfg(test)]
#[path = "load_worker_tests.rs"]
mod tests;
