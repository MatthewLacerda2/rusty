//! stored-format: `cache/bake-queue.json`, the bake queue a cut process leaves behind
//!
//! Written whole, through a temp file and a rename; a file that does not parse is read
//! as no queue, never a panic.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const QUEUE_PATH: &str = "cache/bake-queue.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Pending,
    Running,
    Done,
    Failed,
}

impl State {
    pub fn name(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub path: String,
    pub state: State,
    #[serde(default)]
    pub reason: Option<String>,
    /// Bakes started.
    #[serde(default)]
    pub attempts: u32,
    /// Unix milliseconds.
    #[serde(default)]
    pub started_ms: Option<u64>,
    #[serde(default)]
    pub finished_ms: Option<u64>,
}

impl Entry {
    pub fn pending(path: String) -> Self {
        Self {
            path,
            state: State::Pending,
            reason: None,
            attempts: 0,
            started_ms: None,
            finished_ms: None,
        }
    }

    pub fn summary(&self) -> String {
        match &self.reason {
            Some(why) => format!("{}: {} ({why})", self.path, self.state.name()),
            None => format!("{}: {}", self.path, self.state.name()),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Queue {
    /// The last process to run it; the lock says whether it lives.
    #[serde(default)]
    pub owner: Option<u32>,
    pub scenes: Vec<Entry>,
}

impl Queue {
    pub fn new(paths: Vec<String>) -> Self {
        Self {
            owner: Some(std::process::id()),
            scenes: paths.into_iter().map(Entry::pending).collect(),
        }
    }

    pub fn unfinished(&self) -> bool {
        let open = |e: &Entry| matches!(e.state, State::Pending | State::Running);
        self.scenes.iter().any(open)
    }

    pub fn failed(&self) -> usize {
        let failed = |e: &&Entry| e.state == State::Failed;
        self.scenes.iter().filter(failed).count()
    }
}

#[derive(Debug, PartialEq)]
pub enum Read {
    Missing,
    Found(Queue),
    /// Truncated, not JSON, or an unknown state: why.
    Unreadable(String),
}

pub fn queue_path(root: &Path) -> PathBuf {
    root.join(QUEUE_PATH)
}

pub fn read(root: &Path) -> Read {
    let path = queue_path(root);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Read::Missing,
        Err(e) => return Read::Unreadable(format!("{}: {e}", path.display())),
    };
    match serde_json::from_str(&text) {
        Ok(queue) => Read::Found(queue),
        Err(e) => Read::Unreadable(format!("{}: {e}; read as no queue", path.display())),
    }
}

pub fn write(root: &Path, queue: &Queue) -> Result<(), String> {
    let path = queue_path(root);
    let json = serde_json::to_string_pretty(queue).map_err(|e| e.to_string())?;
    let parent = path.parent().unwrap_or(root);
    std::fs::create_dir_all(parent)
        // lint: allow(asset-write) the bake queue's record, read by the queue alone
        .and_then(|()| rusty_core::fs::write_atomic(&path, json.as_bytes()))
        .map_err(|e| format!("{}: {e}", path.display()))
}

pub fn now_ms() -> u64 {
    crate::frame_log::now_ms()
}
