//! One queue, one owner: the process baking it holds `cache/bake-queue.lock`. The OS
//! frees the lock when that process ends, a kill or a power cut included, so a scene
//! found `running` with the lock free is one a dead process left.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;

use super::file::{read, Read};

pub const LOCK_PATH: &str = "cache/bake-queue.lock";

/// Held for as long as a process bakes the queue; dropping it frees the queue.
#[derive(Debug)]
pub struct Owner {
    _lock: File,
}

impl Owner {
    /// Refuses, naming the holder, while another process (or another run in this one)
    /// bakes the queue.
    pub fn claim(root: &Path) -> Result<Self, String> {
        let path = root.join(LOCK_PATH);
        let parent = path.parent().unwrap_or(root);
        let open = std::fs::create_dir_all(parent).and_then(|()| {
            // lint: allow(asset-write) the bake queue's lock file, never read as content
            OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .open(&path)
        });
        let lock = open.map_err(|e| format!("{}: {e}", path.display()))?;
        match lock.try_lock() {
            Ok(()) => Ok(Self { _lock: lock }),
            Err(TryLockError::WouldBlock) => Err(held(root)),
            Err(TryLockError::Error(e)) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// Whether some live process bakes the queue now, without taking it.
    pub fn is_held(root: &Path) -> bool {
        matches!(Self::claim(root), Err(why) if why.starts_with(HELD))
    }
}

const HELD: &str = "the bake queue is already being baked";

fn held(root: &Path) -> String {
    let by = match read(root) {
        Read::Found(q) => q.owner.map(|pid| match pid == std::process::id() {
            true => " by this process".to_string(),
            false => format!(" by process {pid}"),
        }),
        _ => None,
    };
    format!(
        "{HELD}{}; wait for it to end, or cancel it there",
        by.unwrap_or_default()
    )
}
