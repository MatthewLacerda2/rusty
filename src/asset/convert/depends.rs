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
//! each path's digest with the file's length and modification time, and reads it
//! again only when either changes (a file appearing or vanishing included). A file
//! the engine itself wrote is read again whatever its metadata: each look first
//! drains the write log, `core::fs::written`, and forgets the paths it names
//! (#1219). Play's refresh looks at every source this way too ([`digest`], #1253),
//! so an unchanged project costs one `stat` per source.
//!
//! The modification time is file metadata, not a clock read: it decides only
//! whether to read the file again, never what the sim sees, which is the digest.
//!
//! Only a regular file is read: a texture path naming a folder, a FIFO or
//! `/dev/zero` is absent, never read (which would hang or never end).

use super::ImportError;
use crate::asset::gltf::uri;
use crate::core::fs::written::written_since;
use crate::core::{collections::Map, hash};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::Duration;

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
        let noted = Noted::new(paths);
        if !noted.0.is_empty() {
            self.files
                .retain(|path, look| !noted.names(path, look.canonical.as_deref()));
        }
    }
}

/// Paths the write log named, each with its canonical form: one canonicalization
/// per note, matched against the one each look stored, never one per look.
struct Noted(Vec<(PathBuf, Option<PathBuf>)>);

impl Noted {
    fn new(paths: Vec<String>) -> Self {
        let canonical = |p: &String| std::fs::canonicalize(p).ok();
        Self(paths.iter().map(|p| (p.into(), canonical(p))).collect())
    }

    /// Whether a note names the file at `path` (canonically `canonical`), by the
    /// same spelling or another.
    fn names(&self, path: &Path, canonical: Option<&Path>) -> bool {
        self.0
            .iter()
            .any(|(p, c)| p == path || (c.is_some() && c.as_deref() == canonical))
    }
}

struct Look {
    len: u64,
    modified: Option<Duration>,
    digest: u64,
    /// The file's canonical path, which a note by another spelling matches.
    canonical: Option<PathBuf>,
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

    /// The file at `path`, which a look found but which would not open: unreadable,
    /// whatever a look cached by its length and time says (#1293). A later check
    /// that finds it readable re-converts.
    pub(super) fn unopened(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            seen: Seen::Unreadable,
        }
    }

    /// The file looked at, as the converter named it.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// What the look found.
    pub fn seen(&self) -> Seen {
        self.seen
    }

    /// True when the file was there to read.
    pub fn found(&self) -> bool {
        matches!(self.seen, Seen::Read(_))
    }

    /// The same file, as it reads now.
    pub(super) fn now(&self) -> Self {
        Self {
            path: self.path.clone(),
            seen: seen(&self.path),
        }
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

/// What the file at `path` holds now, by the same memo a dependency's look uses:
/// a source's digest, read again only when its length or mtime changed. A
/// compressed texture's stamp is made from it too (#1574).
pub fn digest(path: &Path) -> Seen {
    seen(path)
}

/// When the file was last modified, as its metadata says: `None` where the
/// platform keeps no such time. Compared for equality, never against a clock.
fn modified(meta: &std::fs::Metadata) -> Option<Duration> {
    let time = meta.modified().ok()?;
    time.duration_since(std::time::UNIX_EPOCH).ok()
}

/// What `path` holds now, read only when this process has not read it at its
/// current length and modification time.
fn seen(path: &Path) -> Seen {
    let Some((len, modified)) = std::fs::metadata(path)
        .ok()
        .filter(std::fs::Metadata::is_file)
        .map(|meta| (meta.len(), modified(&meta)))
    else {
        return Seen::Absent;
    };
    let lock = || LOOKS.lock().unwrap_or_else(PoisonError::into_inner);
    let (known, since) = {
        let mut looks = lock();
        looks.forget_written();
        let known =
            (looks.files.get(path)).filter(|look| look.len == len && look.modified == modified);
        (known.map(|look| look.digest), looks.seen)
    };
    if let Some(digest) = known {
        return Seen::Read(digest);
    }
    // Read without the lock: another thread's look of another file needn't wait.
    let Ok(bytes) = std::fs::read(path) else {
        return Seen::Unreadable;
    };
    let digest = hash::content(&bytes);
    let canonical = std::fs::canonicalize(path).ok();
    let mut looks = lock();
    looks.forget_written();
    // A note taken while the file was read (by another thread's drain, too) means
    // these bytes may be older than the file: answer them, but remember nothing.
    if looks.seen != since && Noted::new(written_since(since).1).names(path, canonical.as_deref()) {
        return Seen::Read(digest);
    }
    let reads = looks.files.get(path).map_or(0, |look| look.reads) + 1;
    // The length read, not the one stat'd: a write between the two is seen next time.
    let len = bytes.len() as u64;
    let look = Look {
        len,
        modified,
        digest,
        canonical,
        reads,
    };
    looks.files.insert(path.to_path_buf(), look);
    Seen::Read(digest)
}

/// How many times this process has read `path`.
#[cfg(test)]
pub(super) fn reads(path: &Path) -> u32 {
    let looks = LOOKS.lock().unwrap_or_else(PoisonError::into_inner);
    looks.files.get(path).map_or(0, |look| look.reads)
}
