//! A change is noted in `core::fs::written` as if the engine wrote it: the caches already
//! follow that log.

pub mod follow;
mod root;

use root::{files_under, Root};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

use notify::event::{AccessKind, AccessMode};
use notify::{Event, EventKind, PollWatcher, RecursiveMode, Watcher};

const POLL_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Backend {
    Native,
    /// Compares modification times in whole seconds: a second save within one second waits
    /// for a later change.
    Poll(Duration),
}

pub struct Watch {
    /// Dropping it stops the watch.
    _watcher: Box<dyn Watcher>,
    events: Receiver<notify::Result<Event>>,
    roots: Vec<Root>,
    pending: BTreeSet<String>,
    warned: bool,
    sources: Option<PathBuf>,
    /// Only the engine writes here, and notes its own writes.
    ignored: Vec<PathBuf>,
}

impl Watch {
    pub fn editor() -> Option<Self> {
        let backend = match std::env::var_os("RUSTY_WATCH_POLL") {
            Some(_) => Backend::Poll(POLL_INTERVAL),
            None => Backend::Native,
        };
        let assets = Path::new(rusty_core::project::ASSETS_DIR);
        // The same path as `shadergen::engine_shader_dir`; `shell` does not depend on `shadergen`.
        let shaders = rusty_core::project::engine_path("shaders");
        let shaders = Path::new(&shaders);
        let roots: Vec<&Path> = [assets, shaders]
            .into_iter()
            .filter(|r| r.is_dir())
            .collect();
        // Before the watch starts: one that fails still has conversions' textures compressed.
        rusty_asset::texture_import::import_in_background();
        let mut watch = Self::start(&roots, backend)
            .map_err(|e| log::warn!("[Editor] not watching the project's files: {e}"))
            .ok()?;
        watch.sources = Some(assets.to_path_buf());
        rusty_asset::texture_import::queue(rusty_asset::texture_import::images(assets));
        watch.ignored = vec![rusty_asset::convert::IMPORT_CACHE_DIR.into()];
        Some(watch)
    }

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
        let deliver = move |event: notify::Result<Event>| {
            if matters(&event) {
                let _ = tx.send(event);
                rusty_core::wake::frame();
            }
        };
        let mut watcher: Box<dyn Watcher> = match backend {
            Backend::Native => Box::new(notify::recommended_watcher(deliver)?),
            Backend::Poll(every) => {
                let config = notify::Config::default().with_poll_interval(every);
                Box::new(PollWatcher::new(deliver, config)?)
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

    fn take(&mut self, event: &Event) {
        if event.need_rescan() || event.paths.is_empty() {
            let mut found = Vec::new();
            for root in &self.roots {
                files_under(&root.given, &mut found);
            }
            return self.pend(found);
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

    fn pend(&mut self, paths: Vec<PathBuf>) {
        for path in paths {
            let temp = path
                .file_name()
                .is_some_and(|n| rusty_core::fs::is_temp_name(&n.to_string_lossy()));
            if temp || self.ignored.iter().any(|dir| path.starts_with(dir)) {
                continue;
            }
            self.pending.insert(path.to_string_lossy().into_owned());
        }
    }

    pub fn apply(&mut self, playing: bool) -> usize {
        self.collect();
        if playing {
            let scripts: Vec<String> = self
                .pending
                .iter()
                .filter(|p| rusty_core::paths::has_extension(p, "lua"))
                .cloned()
                .collect();
            for path in &scripts {
                self.pending.remove(path);
                rusty_core::fs::written::note_written(path);
            }
            return scripts.len();
        }
        let noted: Vec<String> = std::mem::take(&mut self.pending).into_iter().collect();
        for path in &noted {
            rusty_core::fs::written::note_written(path);
        }
        let count = noted.len();
        if let Some(root) = self.sources.as_deref().filter(|_| count > 0) {
            rusty_asset::convert::refresh_changed(root, noted);
        }
        count
    }

    #[cfg(test)]
    pub fn pending(&self) -> &BTreeSet<String> {
        &self.pending
    }
}

fn matters(event: &notify::Result<Event>) -> bool {
    event.as_ref().map_or(true, |e| {
        e.need_rescan() || e.paths.is_empty() || changes(&e.kind)
    })
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
