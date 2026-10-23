//! The references already resolved (#1256): each source's image table, read from
//! its cached `.glb` once and kept until the write log names the source or the
//! `.glb` (a conversion notes both, `stage::note`).
//!
//! Without it every texture load of a reference re-opened the `.glb` and
//! re-parsed its JSON, and the renderer's miss retry did so again every
//! `MISS_RETRY_FRAMES`. A `.glb` that cannot be read is cached as read too, and
//! kept as a [`Freshness`] miss under its own path, so it is read again once the
//! file appears or a conversion writes it.
//!
//! One table per thread: the renderer and a bake on another thread each keep
//! their own, and neither ever locks.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::super::{cache_location, glb};
use crate::asset::gltf::uri;
use crate::core::fs::freshness::Freshness;
use crate::core::fs::written::Stale;

/// One source's textures: the `.glb` they were read from, and each identity's
/// file (`None`: the `.glb` could not be read).
struct Table {
    glb: String,
    files: Option<BTreeMap<String, PathBuf>>,
}

#[derive(Default)]
struct Resolved {
    freshness: Freshness,
    by_source: BTreeMap<String, Table>,
}

thread_local! {
    static RESOLVED: RefCell<Resolved> = RefCell::new(Resolved::default());
}

/// The file `identity` names in `source`'s import cache, through this thread's
/// table.
pub(super) fn file(source: &str, identity: &str) -> Option<PathBuf> {
    RESOLVED.with_borrow_mut(|resolved| {
        resolved.forget_stale();
        resolved
            .table(source)?
            .files
            .as_ref()?
            .get(identity)
            .cloned()
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
    /// Drop the tables whose source or `.glb` was written, or whose missing `.glb`
    /// has appeared. Allocates nothing when nothing was.
    fn forget_stale(&mut self) {
        let changes = self.freshness.poll();
        if changes.stale.is_empty() {
            return;
        }
        let stale = Stale::new(changes.stale);
        self.freshness.forget_misses(&stale);
        self.by_source
            .retain(|source, t| !stale.matches(source) && !stale.matches(&t.glb));
    }

    /// `source`'s table, read now if it is not held. `None` when the source has no
    /// cache location.
    fn table(&mut self, source: &str) -> Option<&Table> {
        if !self.by_source.contains_key(source) {
            let table = read(source)?;
            if table.files.is_none() {
                self.freshness.miss(&table.glb);
            }
            self.by_source.insert(source.to_string(), table);
        }
        self.by_source.get(source)
    }
}

/// `source`'s table, read from its cached `.glb`.
fn read(source: &str) -> Option<Table> {
    #[cfg(test)]
    READS.set(READS.get() + 1);
    let glb = cache_location(Path::new(source)).ok()?;
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
