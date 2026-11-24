//! The queue's steps: each scene marked `running` before its bake and its end after,
//! the file rewritten each time. The headless verbs loop them; the editor steps them.

use std::path::{Path, PathBuf};

use super::file::{now_ms, read, write, Queue, Read, State};
use super::owner::Owner;

#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Done(String),
    Failed(String),
    /// The scene and the rest stay `pending`; the run ends.
    Cancelled,
}

/// The most starts a scene cut under its bake gets.
pub const STARTS: u32 = 2;

pub struct Runner {
    root: PathBuf,
    queue: Queue,
    cancelled: bool,
    _owner: Owner,
}

pub enum Resumed {
    Runner(Runner),
    Nothing(String),
}

impl Runner {
    pub fn start(root: &Path, paths: Vec<String>) -> Result<Self, String> {
        let owner = Owner::claim(root)?;
        let runner = Self {
            root: root.to_path_buf(),
            queue: Queue::new(paths),
            cancelled: false,
            _owner: owner,
        };
        runner.save()?;
        Ok(runner)
    }

    /// A scene found `running` was cut under its bake: retried, then `failed`.
    pub fn resume(root: &Path) -> Result<Resumed, String> {
        let owner = Owner::claim(root)?;
        let mut queue = match read(root) {
            Read::Found(queue) if queue.unfinished() => queue,
            Read::Found(_) => return Ok(Resumed::Nothing("the bake queue is finished".into())),
            Read::Missing => return Ok(Resumed::Nothing("there is no bake queue".into())),
            Read::Unreadable(why) => return Ok(Resumed::Nothing(why)),
        };
        queue.owner = Some(std::process::id());
        for entry in &mut queue.scenes {
            if entry.state != State::Running {
                continue;
            }
            if entry.attempts >= STARTS {
                entry.state = State::Failed;
                entry.reason = Some(format!(
                    "the process ended during its bake {} times; skipped",
                    entry.attempts
                ));
                entry.finished_ms = Some(now_ms());
            } else {
                entry.state = State::Pending;
            }
        }
        let runner = Self {
            root: root.to_path_buf(),
            queue,
            cancelled: false,
            _owner: owner,
        };
        runner.save()?;
        Ok(Resumed::Runner(runner))
    }

    pub fn start_next(&mut self) -> Result<Option<(usize, String)>, String> {
        if self.cancelled {
            return Ok(None);
        }
        let pending = |e: &super::file::Entry| e.state == State::Pending;
        let Some(index) = self.queue.scenes.iter().position(pending) else {
            return Ok(None);
        };
        let entry = &mut self.queue.scenes[index];
        entry.state = State::Running;
        entry.attempts += 1;
        entry.reason = None;
        entry.started_ms = Some(now_ms());
        entry.finished_ms = None;
        let path = entry.path.clone();
        self.save()?;
        Ok(Some((index, path)))
    }

    pub fn finish(&mut self, index: usize, outcome: Outcome) -> Result<(), String> {
        let Some(entry) = self.queue.scenes.get_mut(index) else {
            return Ok(());
        };
        match outcome {
            Outcome::Done(summary) => {
                entry.state = State::Done;
                entry.reason = Some(summary);
            }
            Outcome::Failed(why) => {
                entry.state = State::Failed;
                entry.reason = Some(why);
            }
            Outcome::Cancelled => {
                // No failure: it spends no start.
                entry.state = State::Pending;
                entry.attempts = entry.attempts.saturating_sub(1);
                entry.reason = None;
                entry.started_ms = None;
                self.cancelled = true;
            }
        }
        if entry.state != State::Pending {
            entry.finished_ms = Some(now_ms());
        }
        self.save()
    }

    pub fn run_to_end(&mut self, bake: &mut dyn FnMut(&str) -> Outcome) -> Result<(), String> {
        while let Some((index, path)) = self.start_next()? {
            let outcome = bake(&path);
            self.finish(index, outcome)?;
        }
        Ok(())
    }

    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn was_cancelled(&self) -> bool {
        self.cancelled
    }

    pub fn summary(&self) -> String {
        let q = &self.queue;
        let done = q.scenes.iter().filter(|e| e.state == State::Done).count();
        let mut lines: Vec<String> = q.scenes.iter().map(|e| e.summary()).collect();
        let head = match self.cancelled {
            true => format!(
                "Bake queue cancelled: {done} of {} scene(s) done; Resume continues it",
                q.scenes.len()
            ),
            false => format!(
                "Bake queue ended: {done} of {} scene(s) done, {} failed",
                q.scenes.len(),
                q.failed()
            ),
        };
        lines.insert(0, head);
        lines.join("\n")
    }

    fn save(&self) -> Result<(), String> {
        write(&self.root, &self.queue)
    }
}
