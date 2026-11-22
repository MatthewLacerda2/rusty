use std::path::Path;

use rusty_core::hash::content;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Stamp {
    len: u64,
    pub(super) hash: u64,
}

impl Stamp {
    pub(super) fn of(bytes: &[u8]) -> Self {
        Self {
            len: bytes.len() as u64,
            hash: content(bytes),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Disk {
    Missing,
    File(Stamp),
    Unreadable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Change {
    Same,
    Newer,
    Removed,
}

pub(super) fn change(path: &Path, disk: Disk) -> Change {
    let meta = std::fs::metadata(path);
    let gone = meta
        .as_ref()
        .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound);
    let same = match disk {
        _ if gone => {
            return if disk == Disk::Missing {
                Change::Same
            } else {
                Change::Removed
            }
        }
        Disk::Missing => false,
        Disk::File(s) => {
            meta.is_ok_and(|m| m.is_file() && m.len() == s.len)
                && std::fs::read(path).is_ok_and(|bytes| content(&bytes) == s.hash)
        }
        // Still unreadable: nothing to tell it by, so it is as it was.
        Disk::Unreadable => std::fs::read(path).is_err(),
    };
    if same {
        Change::Same
    } else {
        Change::Newer
    }
}

/// The one spelling a path is recorded under, absolute and folded. No disk read: a symlink
/// stays its own spelling.
pub(super) fn key(path: &Path) -> std::path::PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    rusty_core::paths::fold(&absolute)
}
