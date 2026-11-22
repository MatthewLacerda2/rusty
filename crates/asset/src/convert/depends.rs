//! Each path's digest is remembered with its length and mtime and read again only
//! when either changes, or when the engine wrote it (`core::fs::written`). Only a
//! regular file is read: a folder, FIFO or `/dev/zero` is absent.

use super::ImportError;
use crate::gltf::uri;
use rusty_core::fs::written::written_since;
use rusty_core::{collections::Map, hash};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
pub struct Dependency {
    path: PathBuf,
    seen: Seen,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seen {
    Absent,
    Unreadable,
    Read(u64),
}

static LOOKS: LazyLock<Mutex<Looks>> = LazyLock::new(|| {
    Mutex::new(Looks {
        seen: written_since(u64::MAX).0,
        files: Map::default(),
    })
});

struct Looks {
    seen: u64,
    files: Map<PathBuf, Look>,
}

impl Looks {
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

struct Noted(Vec<(PathBuf, Option<PathBuf>)>);

impl Noted {
    fn new(paths: Vec<String>) -> Self {
        let canonical = |p: &String| std::fs::canonicalize(p).ok();
        Self(paths.iter().map(|p| (p.into(), canonical(p))).collect())
    }

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
    canonical: Option<PathBuf>,
    reads: u32,
}

impl Dependency {
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

    pub(super) fn unopened(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            seen: Seen::Unreadable,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn seen(&self) -> Seen {
        self.seen
    }

    pub fn found(&self) -> bool {
        matches!(self.seen, Seen::Read(_))
    }

    pub(super) fn now(&self) -> Self {
        Self {
            path: self.path.clone(),
            seen: seen(&self.path),
        }
    }

    pub(super) fn holds(&self) -> bool {
        seen(&self.path) == self.seen
    }

    /// `depends <digest, absent or unreadable> <path>`, the path percent-encoded.
    pub(super) fn line(&self) -> String {
        let seen = match self.seen {
            Seen::Absent => ABSENT.to_string(),
            Seen::Unreadable => UNREADABLE.to_string(),
            Seen::Read(digest) => format!("{digest:016x}"),
        };
        let path = uri::encode_path(&self.path.to_string_lossy());
        format!("{DEPENDS}{seen} {path}")
    }

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

pub fn digest(path: &Path) -> Seen {
    seen(path)
}

/// Compared for equality only, never against a clock.
fn modified(meta: &std::fs::Metadata) -> Option<Duration> {
    let time = meta.modified().ok()?;
    time.duration_since(std::time::UNIX_EPOCH).ok()
}

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
    let Ok(bytes) = std::fs::read(path) else {
        return Seen::Unreadable;
    };
    let digest = hash::content(&bytes);
    let canonical = std::fs::canonicalize(path).ok();
    let mut looks = lock();
    looks.forget_written();
    // A write noted while reading means these bytes may be stale: answer, remember nothing.
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

#[cfg(test)]
pub(super) fn reads(path: &Path) -> u32 {
    let looks = LOOKS.lock().unwrap_or_else(PoisonError::into_inner);
    looks.files.get(path).map_or(0, |look| look.reads)
}
