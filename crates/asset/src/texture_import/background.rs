//! The editor's background import (#1574): a worker thread drains a queue of
//! images ([`queue`]), so the editor stays responsive while it encodes. What
//! reaches it: the watch's changes ([`queue_changed`]), the project's images at
//! open, and the textures a conversion extracted, queued where the conversion
//! lands ([`queue_extracted`], #2365) on a thread that imports in the background
//! ([`import_in_background`]).

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
    /// Whether this thread's conversions queue their extracted textures.
    static BACKGROUND: Cell<bool> = const { Cell::new(false) };
}

/// From now on, a model this thread converts has its extracted textures queued
/// (`queue_extracted`): the editor's, whose scene loads, instancing and API
/// imports convert on its main thread. Elsewhere (`make import`, an export, a
/// test) the caller makes them itself with `refresh_all`, on its own thread; the
/// editor's Play refresh does too, and the cache's lock keeps it and the worker
/// from making one copy twice at once.
pub fn import_in_background() {
    BACKGROUND.set(true);
}

/// Queue the textures extracted beside the cached glTF at `glb`, which a
/// conversion just swapped in (`convert::refresh`), when this thread imports in
/// the background ([`import_in_background`]).
pub(crate) fn queue_extracted(glb: &Path) {
    if BACKGROUND.get() {
        queue(extracted::images(glb));
    }
}

#[cfg(test)]
thread_local! {
    static QUEUED: std::cell::RefCell<Vec<PathBuf>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// What this thread has sent the worker since the last call (tests).
#[cfg(test)]
pub(crate) fn take_queued() -> Vec<PathBuf> {
    QUEUED.take()
}

/// [`queue`] every image file under `root` that `changed` names, or whose sidecar
/// it names: the editor watch's refresh (`convert::refresh_changed`). Only the
/// files the watch sees (`refresh::files`): a texture a model extracted lies in
/// the import cache the watch ignores, and is queued by its conversion instead
/// (`queue_extracted`).
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

/// The worker's inbox, started by the first [`queue`].
static WORKER: Mutex<Option<Sender<PathBuf>>> = Mutex::new(None);

/// [`super::refresh`] each of `sources` on a worker thread, in order, on half the cores,
/// so the editor stays responsive while it encodes; each one re-made is noted in `core::fs::written`,
/// so the renderer swaps its uncompressed upload for it. Nothing is queued while
/// imports are refused (Play, `convert::conversion_allowed`): Play's own refresh
/// ran before it, and a change seen during it is queued after Stop.
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
            // The worker is gone (it panicked): the next queue starts another.
            log::warn!("[Import] the texture import worker stopped; textures stay uncompressed");
            *worker = None;
            return;
        }
    }
}

/// The worker's work on one queued `source`. A source gone since it was queued
/// is passed over without an error: a second conversion of its model removed its
/// folder and queued the new one's images in its place.
pub(super) fn import_queued(source: &Path) {
    // Checked first too, so its settings (an extracted image's slots) are not
    // asked of a folder already gone; the match below covers a removal after it.
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
            // The console only shows it on a frame (#1721).
            rusty_core::wake::frame();
        }
    }
}

/// Start the worker: it refreshes what it is sent until the process ends.
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
