//! src/shell/editor/watch/ — the editor's watch on the project's files (#1327).
//!
//! Unity refreshes an asset changed outside it (a Blender re-export, an image
//! editor, a text editor) without a restart. rusty's caches already follow every
//! file the engine writes through one log, `core::fs::written`: the renderer's
//! textures and surface/UI shaders, the import cache's dependencies, the entities
//! using a model (`scene::model_writes`), the script inspector's schemas. So the
//! watch adds no second mechanism: each change `notify` reports under a watched
//! root is **noted in that log as if the engine had written it**, and every cache
//! reloads it the way it reloads a bake.
//!
//! - **Coalesced per frame, on the main thread.** `notify` delivers on its own
//!   thread into a channel; [`Watch::apply`] drains it once per editor frame, so
//!   a save that fires several events notes its path once.
//! - **Only touched sources re-convert.** A frame that noted anything re-converts
//!   the model sources among the notes, and those whose last conversion read a
//!   noted file (an FBX's texture) (`asset::convert::refresh_changed`); the swap's
//!   note reloads every entity using it. A source whose conversion failed waits
//!   until it, or a file it read, changes again: never a Blender run per frame.
//! - **Folders and lost events.** A folder that appears or moves notes every file
//!   under it; an event asking for a rescan (inotify's queue overflowed) notes every
//!   file under the roots.
//! - **Held through Play.** A change seen while playing is kept, and noted on the
//!   first frame after Stop, as conversion already waits for Stop (#1253).
//! - **A polling fallback.** When the native backend (inotify, FSEvents) cannot
//!   start, or `RUSTY_WATCH_POLL` is set (a network or container filesystem that
//!   gives no events), `notify`'s `PollWatcher` compares the files every second, by
//!   modification time to the second: a second save within the same second as the
//!   one before it waits for the next change to be seen.
//! - **Spelled as the caches spell it.** A path is noted relative to the working
//!   directory (the project root) when it lies under a root given that way, so a
//!   deleted file still matches its cache key by name. The native backends report
//!   absolute paths and the poller keeps the root as given; both are spelled alike
//!   (`root`).
//!
//! Editor only: a shipped game never watches its files. The engine's own
//! write-in-flight temp files (`core::fs::is_temp_name`) and the import cache
//! (`asset::convert::IMPORT_CACHE_DIR`, which only conversions write, and note) are
//! never noted.

pub mod follow;
mod root;

use root::{files_under, Root};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

use notify::event::{AccessKind, AccessMode};
use notify::{Event, EventKind, PollWatcher, RecursiveMode, Watcher};

/// How often the polling fallback compares the watched files.
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Which `notify` backend watches.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Backend {
    /// The platform's own events (inotify, FSEvents), falling back to polling.
    Native,
    /// Compare the files every interval.
    Poll(Duration),
}

/// A running watch: its watcher, the events it delivered, the paths not yet noted.
pub struct Watch {
    /// Kept alive for as long as the watch runs; dropping it stops the thread.
    _watcher: Box<dyn Watcher>,
    events: Receiver<notify::Result<Event>>,
    roots: Vec<Root>,
    pending: BTreeSet<String>,
    /// Whether the last watcher error was already logged.
    warned: bool,
    /// The folder whose converted sources are refreshed after a change, as Play's
    /// refresh does; `None` refreshes nothing.
    sources: Option<PathBuf>,
    /// Folders under a root whose changes are never noted: only the engine writes
    /// there, and notes its own writes.
    ignored: Vec<PathBuf>,
}

impl Watch {
    /// The editor's watch: the project's `assets/` (the working directory is the
    /// project root) and the engine's shader set, which surface variants also
    /// resolve against. `None`, logged, when nothing could be watched.
    pub fn editor() -> Option<Self> {
        let backend = match std::env::var_os("RUSTY_WATCH_POLL") {
            Some(_) => Backend::Poll(POLL_INTERVAL),
            None => Backend::Native,
        };
        let assets = Path::new(crate::core::project::ASSETS_DIR);
        // `shadergen::engine_shader_dir`'s path, named through `core` (the layer table).
        let shaders = crate::core::project::engine_path("shaders");
        let shaders = Path::new(&shaders);
        let roots: Vec<&Path> = [assets, shaders]
            .into_iter()
            .filter(|r| r.is_dir())
            .collect();
        let mut watch = Self::start(&roots, backend)
            .map_err(|e| log::warn!("[Editor] not watching the project's files: {e}"))
            .ok()?;
        watch.sources = Some(assets.to_path_buf());
        watch.ignored = vec![crate::asset::convert::IMPORT_CACHE_DIR.into()];
        Some(watch)
    }

    /// Watch every folder in `roots`, recursively. A native backend that fails to
    /// start or to watch a root falls back to polling.
    pub fn start(roots: &[&Path], backend: Backend) -> notify::Result<Self> {
        match backend {
            Backend::Native => Self::with(roots, Backend::Native).or_else(|e| {
                log::info!("[Editor] file events unavailable ({e}); polling instead");
                Self::with(roots, Backend::Poll(POLL_INTERVAL))
            }),
            poll => Self::with(roots, poll),
        }
    }

    fn with(roots: &[&Path], backend: Backend) -> notify::Result<Self> {
        let (tx, events) = channel();
        let mut watcher: Box<dyn Watcher> = match backend {
            Backend::Native => Box::new(notify::recommended_watcher(tx)?),
            Backend::Poll(every) => {
                let config = notify::Config::default().with_poll_interval(every);
                Box::new(PollWatcher::new(tx, config)?)
            }
        };
        for root in roots {
            watcher.watch(root, RecursiveMode::Recursive)?;
        }
        Ok(Self {
            _watcher: watcher,
            events,
            roots: roots.iter().map(|r| Root::new(r)).collect(),
            pending: BTreeSet::new(),
            warned: false,
            sources: None,
            ignored: Vec::new(),
        })
    }

    /// Drain the delivered events into the pending set.
    pub fn collect(&mut self) {
        while let Ok(event) = self.events.try_recv() {
            match event {
                Ok(event) => self.take(&event),
                Err(e) if !self.warned => {
                    self.warned = true;
                    log::warn!("[Editor] the file watch reported: {e}");
                }
                Err(_) => {}
            }
        }
    }

    /// Note what one event changed. A folder notes every file under it (a folder
    /// moved in or out); an event that asks for a rescan, or names no path (inotify
    /// overflowed, events were lost), notes every file under the roots.
    fn take(&mut self, event: &Event) {
        if event.need_rescan() || event.paths.is_empty() {
            let mut found = Vec::new();
            for root in &self.roots {
                files_under(&root.given, &mut found);
            }
            return self.pend(found);
        }
        if !changes(&event.kind) {
            return;
        }
        let mut found = Vec::new();
        for path in &event.paths {
            match self.roots.iter().find_map(|r| r.spell(path)) {
                Some(spelled) if spelled.is_dir() => files_under(&spelled, &mut found),
                Some(spelled) => found.push(spelled),
                None => found.push(path.clone()),
            }
        }
        self.pend(found);
    }

    /// Add `paths`, already spelled, to the pending set, but for write-in-flight temp
    /// files and paths under an ignored folder.
    fn pend(&mut self, paths: Vec<PathBuf>) {
        for path in paths {
            let temp = path
                .file_name()
                .is_some_and(|n| crate::core::fs::is_temp_name(&n.to_string_lossy()));
            if temp || self.ignored.iter().any(|dir| path.starts_with(dir)) {
                continue;
            }
            self.pending.insert(path.to_string_lossy().into_owned());
        }
    }

    /// Once per frame: drain the events, and unless `playing`, note every pending
    /// path in the write log, then re-convert the sources the notes touched (a
    /// texture an FBX's conversion read is no model path, so only this re-converts
    /// it). Returns how many were noted.
    pub fn apply(&mut self, playing: bool) -> usize {
        self.collect();
        if playing {
            return 0;
        }
        let noted: Vec<String> = std::mem::take(&mut self.pending).into_iter().collect();
        for path in &noted {
            crate::core::fs::written::note_written(path);
        }
        let count = noted.len();
        if let Some(root) = self.sources.as_deref().filter(|_| count > 0) {
            crate::asset::convert::refresh_changed(root, noted);
        }
        count
    }

    /// Paths seen and not yet noted.
    #[cfg(test)]
    pub fn pending(&self) -> &BTreeSet<String> {
        &self.pending
    }
}

/// Whether an event of `kind` can change what a file holds: everything but a
/// read-only open, read or close.
fn changes(kind: &EventKind) -> bool {
    match kind {
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => true,
        EventKind::Access(_) => false,
        _ => true,
    }
}

#[cfg(test)]
mod tests;
