//! Each source's image table, read once from its cached `.glb` and kept until the
//! write log names the source or the `.glb`.
//!
//! - Keys are absolute, so opening another project (`core::project::moves`) drops
//!   every table and never serves the old project's textures.
//! - An unreadable `.glb` is held as unread with its size and mtime; a lookup
//!   re-reads it only when those change.
//! - Another process's writes are not seen until the engine converts again.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use super::super::{cache_location, glb};
use crate::gltf::uri;
use rusty_core::fs::written::{written_since, Stale};
use rusty_core::project;

struct Table {
    glb: String,
    files: Option<BTreeMap<String, PathBuf>>,
    stamp: Option<Stamp>,
}

type Stamp = (u64, Option<Duration>);

fn stamp(glb: &str) -> Option<Stamp> {
    let meta = std::fs::metadata(glb).ok()?;
    let modified = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok();
    Some((meta.len(), modified))
}

#[derive(Default)]
struct Resolved {
    seen: u64,
    project: Option<(u64, PathBuf)>,
    by_source: BTreeMap<String, Table>,
}

thread_local! {
    static RESOLVED: RefCell<Resolved> = RefCell::new(Resolved::default());
}

pub(super) fn file(source: &str, identity: &str) -> Option<PathBuf> {
    RESOLVED.with_borrow_mut(|resolved| {
        resolved.enter(project::moves(), project::root);
        resolved.file(source, identity)
    })
}

#[cfg(test)]
pub(super) fn file_in(moves: u64, root: &Path, source: &str, identity: &str) -> Option<PathBuf> {
    RESOLVED.with_borrow_mut(|resolved| {
        resolved.enter(moves, || root.to_path_buf());
        resolved.file(source, identity)
    })
}

#[cfg(test)]
thread_local! {
    static READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(super) fn reads() -> usize {
    READS.get()
}

impl Resolved {
    fn enter(&mut self, moves: u64, root: impl FnOnce() -> PathBuf) {
        if self.project.as_ref().is_some_and(|(m, _)| *m == moves) {
            return;
        }
        self.by_source.clear();
        self.seen = written_since(u64::MAX).0;
        self.project = Some((moves, root()));
    }

    fn file(&mut self, source: &str, identity: &str) -> Option<PathBuf> {
        self.forget_written();
        let root = self.project.as_ref().map(|(_, r)| r.clone())?;
        self.table(&root, absolute(&root, source))?
            .files
            .as_ref()?
            .get(identity)
            .cloned()
    }

    fn forget_written(&mut self) {
        let (seen, written) = written_since(self.seen);
        self.seen = seen;
        if written.is_empty() {
            return;
        }
        let stale = Stale::new(written);
        self.by_source
            .retain(|source, t| !stale.matches(source) && !stale.matches(&t.glb));
    }

    fn table(&mut self, root: &Path, key: String) -> Option<&Table> {
        let unread =
            |t: &Table| t.files.is_none() && stamp(&t.glb).is_some_and(|s| Some(s) != t.stamp);
        if self.by_source.get(&key).is_none_or(unread) {
            let table = read(root, Path::new(&key))?;
            self.by_source.insert(key.clone(), table);
        }
        self.by_source.get(&key)
    }
}

fn absolute(root: &Path, source: &str) -> String {
    let joined = root.join(source);
    let parts = joined.components().filter(|c| *c != Component::CurDir);
    parts.collect::<PathBuf>().to_string_lossy().into_owned()
}

/// `source` is absolute; its cached `.glb`'s path is made absolute under `root` too.
fn read(root: &Path, source: &Path) -> Option<Table> {
    #[cfg(test)]
    READS.set(READS.get() + 1);
    let glb = PathBuf::from(absolute(
        root,
        &cache_location(source).ok()?.to_string_lossy(),
    ));
    // Where the glTF importer anchors these URIs, so both resolve one file.
    let base_dir = glb.parent().unwrap_or_else(|| Path::new(""));
    let files = glb::image_uris(&glb).map(|uris| {
        uris.into_iter()
            .filter_map(|(name, u)| Some((name, uri::file(base_dir, &u)?)))
            .collect()
    });
    let glb = glb.to_string_lossy().into_owned();
    let stamp = stamp(&glb);
    Some(Table { glb, files, stamp })
}
