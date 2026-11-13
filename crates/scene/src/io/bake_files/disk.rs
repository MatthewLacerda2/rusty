//! What a bake file's path held, as [`BakeFiles`](super::BakeFiles) compares it:
//! the file's length, then its content hash (`core::hash`), never a clock.

use std::path::Path;

use rusty_core::hash::content;

/// A file's bytes, as compared: their length and content hash.
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

/// What this world found at a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Disk {
    Missing,
    File(Stamp),
    /// There, but it could not be read (a permission, a directory in its place).
    Unreadable,
}

/// How the file at a path compares with what this world recorded there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Change {
    Same,
    /// Rewritten or created by someone else since.
    Newer,
    /// Removed outside the engine since.
    Removed,
}

/// How the file at `path` compares with `disk`, what this world recorded there: a
/// metadata call, and a read and hash only when the length still matches.
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

/// The one spelling a path is recorded under: absolute, with every `.` and `..`
/// folded (`core::paths::fold`), so `scenes/level.scene.occlusion` and its absolute
/// spelling find one record. No disk read: a symlink stays its own spelling.
pub(super) fn key(path: &Path) -> std::path::PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    rusty_core::paths::fold(&absolute)
}
