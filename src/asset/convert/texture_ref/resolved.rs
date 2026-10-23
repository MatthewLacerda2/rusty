//! The references already resolved (#1256): each source's image table, read from
//! its cached `.glb` once and kept until the write log names the source or the
//! `.glb` (a conversion notes both, `stage::note`).
//!
//! Without it every texture load of a reference re-opened the `.glb` and
//! re-parsed its JSON, and the renderer's miss retry did so again every
//! `MISS_RETRY_FRAMES`.
//!
//! - **Keys are absolute** (#1269): a source is held under its path joined onto
//!   the project root, and its `.glb` and texture files are stored absolute too.
//!   Opening another project (`core::project::moves`) drops every table, so
//!   `models/soldier.fbx` in the new project never finds the old one's textures.
//! - **A `.glb` that cannot be read is held as unread**, and the lookup that next
//!   asks for it checks whether it has appeared (one `Path::exists`) and reads it
//!   if so. The cache keeps no retry beat of its own: the callers remember a miss
//!   and ask again only on theirs (`core::fs::freshness`), so a `.glb` another
//!   process writes is picked up within one caller beat.
//! - **What another process does is otherwise not seen.** A `.glb` it re-writes
//!   after the table was read keeps the old table, a texture it adds included,
//!   until the engine itself converts the source again: the same as every
//!   path-keyed cache (`core::fs::written`).
//!
//! One table per thread: the renderer and a bake on another thread each keep
//! their own. A lookup takes the write log's lock for as long as it takes to see
//! nothing was written (`written_since`), and no other lock.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use super::super::{cache_location, glb};
use crate::asset::gltf::uri;
use crate::core::fs::written::{written_since, Stale};
use crate::core::project;

/// One source's textures: the `.glb` they were read from, and each identity's
/// file (`None`: the `.glb` could not be read).
struct Table {
    glb: String,
    files: Option<BTreeMap<String, PathBuf>>,
}

#[derive(Default)]
struct Resolved {
    /// The write generation already seen.
    seen: u64,
    /// The project the tables were read under: its `project::moves` count and root.
    project: Option<(u64, PathBuf)>,
    /// Each source's table, under its absolute path.
    by_source: BTreeMap<String, Table>,
}

thread_local! {
    static RESOLVED: RefCell<Resolved> = RefCell::new(Resolved::default());
}

/// The file `identity` names in `source`'s import cache, through this thread's
/// table.
pub(super) fn file(source: &str, identity: &str) -> Option<PathBuf> {
    RESOLVED.with_borrow_mut(|resolved| {
        resolved.enter(project::moves(), project::root);
        resolved.file(source, identity)
    })
}

/// [`file`] as the project `moves` at `root` would resolve it (tests switch
/// projects without moving the working directory, which they share).
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

/// How many `.glb` tables this thread has read (tests).
#[cfg(test)]
pub(super) fn reads() -> usize {
    READS.get()
}

impl Resolved {
    /// Follow the open project: a project other than the one the tables were read
    /// under drops them all, and its `root` anchors the keys from now on.
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

    /// Drop the tables whose source or `.glb` was written. Allocates nothing when
    /// nothing was.
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

    /// The table of the source at `key`, read now if it is not held or its `.glb`
    /// was missing and has appeared. `None` when the source has no cache location.
    fn table(&mut self, root: &Path, key: String) -> Option<&Table> {
        let unread = |t: &Table| t.files.is_none() && Path::new(&t.glb).exists();
        if self.by_source.get(&key).is_none_or(unread) {
            let table = read(root, Path::new(&key))?;
            self.by_source.insert(key.clone(), table);
        }
        self.by_source.get(&key)
    }
}

/// `source` joined onto `root`, without its `.` components.
fn absolute(root: &Path, source: &str) -> String {
    let joined = root.join(source);
    let parts = joined.components().filter(|c| *c != Component::CurDir);
    parts.collect::<PathBuf>().to_string_lossy().into_owned()
}

/// The table of the source at the absolute path `source`, read from its cached
/// `.glb` (held under `root` too).
fn read(root: &Path, source: &Path) -> Option<Table> {
    #[cfg(test)]
    READS.set(READS.get() + 1);
    let glb = PathBuf::from(absolute(
        root,
        &cache_location(source).ok()?.to_string_lossy(),
    ));
    // The folder the glTF importer anchors the same URIs at.
    let base_dir = glb.parent().unwrap_or_else(|| Path::new(""));
    let files = glb::image_uris(&glb).map(|uris| {
        uris.into_iter()
            .filter_map(|(name, u)| Some((name, uri::file(base_dir, &u)?)))
            .collect()
    });
    let glb = glb.to_string_lossy().into_owned();
    Some(Table { glb, files })
}
