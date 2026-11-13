//! One path's record in [`BakeFiles`](super::BakeFiles), and what a save would
//! bring that path to.

use std::sync::Arc;

use super::disk::Disk;
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
