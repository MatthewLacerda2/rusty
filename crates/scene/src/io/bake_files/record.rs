use std::path::Path;
use std::sync::Arc;

use super::disk::{key, Disk};
use super::BakeFiles;
use rusty_core::hash::content;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mark {
    Loaded,
    Refused,
    Baked,
}

impl BakeFiles {
    pub fn copied(&self, from: &Path, to: &Path) {
        let Some(mut seen) = self.seen.borrow().get(&key(from)).cloned() else {
            return;
        };
        seen.why = None;
        let to = key(to);
        self.warned.borrow_mut().remove(&to);
        self.seen.borrow_mut().insert(to, seen);
    }
}

#[derive(Clone, Debug)]
pub(super) struct Seen {
    pub(super) disk: Disk,
    pub(super) held: Option<u64>,
    pub(super) buffer: Option<Arc<[u8]>>,
    pub(super) mark: Mark,
    pub(super) why: Option<String>,
}

impl Seen {
    pub(super) fn loaded(disk: Disk, held: Option<u64>, buffer: Option<Arc<[u8]>>) -> Self {
        let (mark, why) = (Mark::Loaded, None);
        Self {
            disk,
            held,
            buffer,
            mark,
            why,
        }
    }
}

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
