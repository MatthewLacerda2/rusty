use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::sync::Mutex;

use super::encode::Cores;
use super::refresh::{files, refresh_on};
use crate::convert::{self, texture_ref::extracted};
use crate::sidecar;
use rusty_core::fs::written::Stale;

thread_local! {
    static BACKGROUND: Cell<bool> = const { Cell::new(false) };
}

/// Queues this thread's conversions' extracted textures; elsewhere the caller runs `refresh_all`.
/// The cache's lock stops Play's refresh and the worker making one copy twice.
pub fn import_in_background() {
    BACKGROUND.set(true);
}

pub(crate) fn queue_extracted(glb: &Path) {
    if BACKGROUND.get() {
        queue(extracted::images(glb));
    }
}

#[cfg(test)]
thread_local! {
    static QUEUED: std::cell::RefCell<Vec<PathBuf>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
pub(crate) fn take_queued() -> Vec<PathBuf> {
    QUEUED.take()
}

/// Only the files the watch sees; a model's extracted textures are queued by its conversion.
pub fn queue_changed(root: &Path, changed: &Stale) {
    if changed.is_empty() {
        return;
    }
    let names = |path: &Path| changed.matches(&path.to_string_lossy());
    let touched = files(root)
        .into_iter()
        .filter(|image| names(image) || names(&sidecar::meta_path(image)));
    queue(touched.collect());
}

static WORKER: Mutex<Option<Sender<PathBuf>>> = Mutex::new(None);

/// Nothing is queued while imports are refused (Play); a change seen then is queued after Stop.
pub fn queue(sources: Vec<PathBuf>) {
    if sources.is_empty() || !convert::conversion_allowed() {
        return;
    }
    #[cfg(test)]
    QUEUED.with_borrow_mut(|q| q.extend(sources.iter().cloned()));
    let mut worker = WORKER.lock().unwrap_or_else(|e| e.into_inner());
    let inbox = worker.get_or_insert_with(start);
    for source in sources {
        if inbox.send(source).is_err() {
            // The worker panicked: the next queue starts another.
            log::warn!("[Import] the texture import worker stopped; textures stay uncompressed");
            *worker = None;
            return;
        }
    }
}

pub(super) fn import_queued(source: &Path) {
    // An extracted image gone since queued: a re-conversion removed its folder. Checked
    // first, so no slot is asked of it; the match covers a later removal.
    if !source.exists() {
        return;
    }
    match refresh_on(source, "", Cores::Half) {
        Ok(true) => {
            let path = source.to_string_lossy();
            rusty_core::fs::written::note_written(&path);
            log::info!("[Import] compressed {path}");
        }
        Ok(false) => {}
        Err(_) if !source.exists() => {}
        Err(e) => {
            log::warn!("[Import] {e}");
            // The console shows it only on a drawn frame.
            rusty_core::wake::frame();
        }
    }
}

fn start() -> Sender<PathBuf> {
    let (inbox, sent) = channel::<PathBuf>();
    let spawned = std::thread::Builder::new()
        .name("texture-import".into())
        .spawn(move || {
            for source in sent {
                import_queued(&source);
            }
        });
    if let Err(e) = spawned {
        log::warn!("[Import] could not start the texture import worker: {e}");
    }
    inbox
}
