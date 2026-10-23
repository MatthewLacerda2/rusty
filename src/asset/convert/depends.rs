//! The files beside a source a conversion looked at (#1189): an FBX's on-disk
//! textures. A converter reports each, found or not, as a [`Dependency`], and the
//! cache is current only while each still reads the same (`record`), so a texture
//! appearing, vanishing or changing re-converts, as Unity reimports an asset when
//! one of its dependencies changes.
//!
//! **Looked at once per change, not once per load.** `import_file` checks the cache
//! once per mesh entity on a scene load, a `Scene.Instantiate` and a prefab
//! propagation, and an atlas may back every material of a model: hashing each
//! texture on each check would read gigabytes per load. So this process remembers
//! each path's digest with the file's length, and reads it again only when the
//! length changes (a file appearing or vanishing included). A file the engine
//! itself wrote is read again whatever its length: each look first drains the
//! write log, `core::fs::written`, and forgets the paths it names (#1219). An
//! outside tool's edit that keeps the length is seen by the next process.
//!
//! Only a regular file is read: a texture path naming a folder, a FIFO or
//! `/dev/zero` is absent, never read (which would hang or never end).

use super::ImportError;
use crate::asset::gltf::uri;
use crate::core::fs::written::{written_since, Stale};
use crate::core::{collections::Map, hash};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};

/// One external file a conversion looked at, as it found it.
#[derive(Clone, Debug, PartialEq)]
pub struct Dependency {
    path: PathBuf,
    seen: Seen,
}

/// What looking at a path found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seen {
    /// No regular file there.
    Absent,
    /// A regular file that could not be read.
    Unreadable,
    /// A regular file, with the digest of its bytes.
    Read(u64),
}

/// This process's looks, by path: the length each was read at, and what it held.
static LOOKS: LazyLock<Mutex<Looks>> = LazyLock::new(|| {
    Mutex::new(Looks {
        seen: written_since(u64::MAX).0,
        files: Map::default(),
    })
});

struct Looks {
    /// The newest write generation already applied to `files`.
    seen: u64,
    files: Map<PathBuf, Look>,
}

impl Looks {
    /// Forget every look at a file the engine wrote since the last call.
    fn forget_written(&mut self) {
        let (seen, paths) = written_since(self.seen);
        self.seen = seen;
        let stale = Stale::new(paths);
        if !stale.is_empty() {
            self.files
                .retain(|path, _| !stale.matches(&path.to_string_lossy()));
        }
    }
}

struct Look {
    len: u64,
    digest: u64,
    /// How many times the file was read: what the tests count.
    reads: u32,
}

impl Dependency {
    /// Look at the file at `path`: what a converter calls before deciding on it,
    /// so what the stamp records is what the conversion saw. A path that is not
    /// UTF-8 is refused, as a source's is: the stamp could not name it exactly.
    pub fn look(path: &Path) -> Result<Self, ImportError> {
        if path.to_str().is_none() {
            return Err(ImportError::Parse(format!(
                "{}: a dependency path must be UTF-8",
                path.display()
            )));
        }
        Ok(Self {
            path: path.to_path_buf(),
            seen: seen(path),
        })
    }

    /// What the look found.
    pub fn seen(&self) -> Seen {
        self.seen
    }

    /// True when the file was there to read.
    pub fn found(&self) -> bool {
        matches!(self.seen, Seen::Read(_))
    }

    /// True when the file still reads as it did.
    pub(super) fn holds(&self) -> bool {
        seen(&self.path) == self.seen
    }

    /// `depends <digest, absent or unreadable> <path>`, the path percent-encoded
    /// so any name fits on one line and reads back exactly.
    pub(super) fn line(&self) -> String {
        let seen = match self.seen {
            Seen::Absent => ABSENT.to_string(),
            Seen::Unreadable => UNREADABLE.to_string(),
            Seen::Read(digest) => format!("{digest:016x}"),
        };
        let path = uri::encode_path(&self.path.to_string_lossy());
        format!("{DEPENDS}{seen} {path}")
    }

    /// The dependency a [`Dependency::line`] names: `None` for a line that is not
    /// one, `Some(None)` for one that does not parse.
    pub(super) fn parse(line: &str) -> Option<Option<Self>> {
        let rest = line.strip_prefix(DEPENDS)?;
        Some(rest.split_once(' ').and_then(|(seen, path)| {
            let seen = match seen {
                ABSENT => Seen::Absent,
                UNREADABLE => Seen::Unreadable,
                hex if hex.len() == 16 => Seen::Read(u64::from_str_radix(hex, 16).ok()?),
                _ => return None,
            };
            let path = PathBuf::from(uri::decode(path)?);
            Some(Self { path, seen })
        }))
    }
}

const DEPENDS: &str = "depends ";
const ABSENT: &str = "absent";
const UNREADABLE: &str = "unreadable";

/// What `path` holds now, read only when this process has not read it at its
/// current length.
fn seen(path: &Path) -> Seen {
    let Some(len) = std::fs::metadata(path)
        .ok()
        .filter(std::fs::Metadata::is_file)
        .map(|meta| meta.len())
    else {
        return Seen::Absent;
    };
    let lock = || LOOKS.lock().unwrap_or_else(PoisonError::into_inner);
    let known = {
        let mut looks = lock();
        looks.forget_written();
        looks
            .files
            .get(path)
            .filter(|look| look.len == len)
            .map(|look| look.digest)
    };
    if let Some(digest) = known {
        return Seen::Read(digest);
    }
    // Read without the lock: another thread's look of another file needn't wait.
    let Ok(bytes) = std::fs::read(path) else {
        return Seen::Unreadable;
    };
    let digest = hash::fnv1a64(&bytes);
    let mut looks = lock();
    let reads = looks.files.get(path).map_or(0, |look| look.reads) + 1;
    // The length read, not the one stat'd: a write between the two is seen next time.
    let len = bytes.len() as u64;
    looks
        .files
        .insert(path.to_path_buf(), Look { len, digest, reads });
    Seen::Read(digest)
}

/// How many times this process has read `path`.
#[cfg(test)]
pub(super) fn reads(path: &Path) -> u32 {
    let looks = LOOKS.lock().unwrap_or_else(PoisonError::into_inner);
    looks.files.get(path).map_or(0, |look| look.reads)
}
