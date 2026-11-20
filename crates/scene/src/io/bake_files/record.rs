//! One path's record in [`BakeFiles`](super::BakeFiles), and what a save would
//! bring that path to.

use std::path::Path;
use std::sync::Arc;

use super::disk::{key, Disk};
use super::BakeFiles;
use rusty_core::hash::content;

/// Why a save may write a path even when this world holds what it held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mark {
    /// Only read or written: what this world holds came from that file.
    Loaded,
    /// Read and refused: rewritten or removed while the file is still that one.
    Refused,
    /// This world baked since: written whatever is on disk (the last write wins).
    Baked,
}

impl BakeFiles {
    /// The file at `from` was copied to `to` byte for byte (none there: none at
    /// either): `to` takes `from`'s record, so a save to `to` does what a save to
    /// `from` would, as `make bench-scenes`' editor round times a save of a loaded
    /// scene outside its project. Nothing without a record at `from`.
    pub fn copied(&self, from: &Path, to: &Path) {
        let Some(mut seen) = self.seen.borrow().get(&key(from)).cloned() else {
            return;
        };
        // What `from`'s load did not use is that load's to tell, not the copy's.
        seen.why = None;
        let to = key(to);
        self.warned.borrow_mut().remove(&to);
        self.seen.borrow_mut().insert(to, seen);
    }
}

/// What this world saw at one path.
#[derive(Clone, Debug)]
pub(super) struct Seen {
    pub(super) disk: Disk,
    /// The hash of what this world would write right after (`None`: no bake), so a
    /// save tells a bake since from a format that does not round-trip byte for byte.
    pub(super) held: Option<u64>,
    /// The shared buffer `held` hashes, kept alive so its address names those bytes:
    /// a save of the same `Arc` is not hashed again.
    pub(super) buffer: Option<Arc<[u8]>>,
    pub(super) mark: Mark,
    /// Why this world did not use the file it read here (`"refused (…)"`, `"could
    /// not be read (…)"`): set only by the record of that read, cleared by any
    /// later record and by a bake.
    pub(super) why: Option<String>,
}

/// What a save would bring a path to: bytes, maybe in a shared buffer.
#[derive(Clone, Copy)]
pub(super) enum Want<'a> {
    Bytes(&'a [u8]),
    Shared(&'a Arc<[u8]>),
}

impl<'a> Want<'a> {
    pub(super) fn bytes(self) -> &'a [u8] {
        match self {
            Self::Bytes(b) => b,
            Self::Shared(a) => a,
        }
    }

    /// Its hash, taken from `seen` when it is the buffer `seen` already hashed.
    pub(super) fn hash(&self, seen: Option<&Seen>) -> u64 {
        let cached = seen.and_then(|s| Some((s.buffer.as_ref()?, s.held?)));
        match (self, cached) {
            (Self::Shared(a), Some((b, h))) if Arc::ptr_eq(a, b) => h,
            _ => content(self.bytes()),
        }
    }

    pub(super) fn buffer(&self) -> Option<Arc<[u8]>> {
        match self {
            Self::Shared(a) => Some(Arc::clone(a)),
            Self::Bytes(_) => None,
        }
    }
}
