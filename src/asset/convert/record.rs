//! A cache's stamp file (`<cache>.glb.source`): the stamp that says which
//! conversion the cache is, then one line per external file it looked at (#1189),
//! then one line per part it skipped (#1172), and the warnings those skips raise.
//!
//! **What a conversion depended on.** A converter reports every file beside its
//! source it looked at (an FBX's on-disk texture), found or not, as a
//! [`Dependency`]: its path and a digest of its bytes, or none when it was
//! missing. The cache is current only while each still reads the same, so a
//! texture appearing, vanishing or changing re-converts, as Unity reimports an
//! asset when one of its dependencies changes.
//!
//! A conversion warns of its skips as it runs. A cache hit replays them **once per
//! process** per cache and stamp: `import_file` runs once per mesh entity on a
//! scene load, a `Scene.Instantiate` and a prefab propagation, and replaying on
//! each would flood the log. A model loaded in a fresh process still says what it
//! is missing, as Unity shows import warnings on every reimport.

use super::{Converted, Converter};
use crate::asset::gltf::uri;
use crate::core::{collections::Set, hash};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};

/// The (cache, stamp) pairs whose skips this process has warned of.
static WARNED: LazyLock<Mutex<Set<(PathBuf, String)>>> = LazyLock::new(Mutex::default);

/// What the stamp records: the converter, its version and the source's digest.
pub(super) fn stamp(converter: &Converter, source: &[u8]) -> String {
    let digest = hash::fnv1a64(source);
    format!("{} {} {digest:016x}\n", converter.name, converter.version)
}

pub(super) fn stamp_path(out: &Path) -> PathBuf {
    super::with_suffix(out, ".source")
}

/// One external file a conversion looked at, as it found it.
#[derive(Clone, Debug, PartialEq)]
pub struct Dependency {
    path: PathBuf,
    /// The digest of its bytes; `None` when it could not be read.
    digest: Option<u64>,
}

impl Dependency {
    /// Look at the file at `path`: what a converter calls before deciding on it.
    pub fn look(path: &Path) -> Self {
        let digest = std::fs::read(path).ok().map(|bytes| hash::fnv1a64(&bytes));
        Self {
            path: path.to_path_buf(),
            digest,
        }
    }

    /// True when the file was there to read.
    pub fn found(&self) -> bool {
        self.digest.is_some()
    }

    /// True when the file still reads as it did.
    fn holds(&self) -> bool {
        Self::look(&self.path).digest == self.digest
    }

    /// `depends <digest or absent> <path>`, the path percent-encoded so any name
    /// fits on one line and reads back exactly.
    fn line(&self) -> String {
        let digest = self
            .digest
            .map_or_else(|| ABSENT.to_string(), |d| format!("{d:016x}"));
        let path = uri::encode_path(&self.path.to_string_lossy());
        format!("{DEPENDS}{digest} {path}")
    }

    /// The dependency a [`Dependency::line`] names; `None` for any other line.
    fn parse(line: &str) -> Option<Self> {
        let (digest, path) = line.strip_prefix(DEPENDS)?.split_once(' ')?;
        let digest = match digest {
            ABSENT => None,
            hex => Some(u64::from_str_radix(hex, 16).ok()?),
        };
        let path = PathBuf::from(uri::decode(path)?);
        Some(Self { path, digest })
    }
}

const DEPENDS: &str = "depends ";
const ABSENT: &str = "absent";

/// What a stamp file holds: the stamp, then one line per file looked at (each
/// once), then one line per skip.
pub(super) fn record(stamp: &str, converted: &Converted) -> String {
    let mut record = stamp.to_string();
    let mut seen = Set::default();
    for dependency in &converted.depends_on {
        if seen.insert(&dependency.path) {
            record.push_str(&dependency.line());
            record.push('\n');
        }
    }
    for skip in &converted.skipped {
        record.push_str(&skip.line());
        record.push('\n');
    }
    record
}

/// A stamp file's stamp (its first line, newline included) and the skips after it.
pub(super) fn split_record(record: &str) -> (&str, &str) {
    let end = record.find('\n').map_or(record.len(), |i| i + 1);
    record.split_at(end)
}

/// The conversion cached at `out`, read back from its stamp file.
struct Stored {
    stamp: String,
    depends_on: Vec<Dependency>,
    skipped: Vec<String>,
}

fn read(out: &Path) -> Option<Stored> {
    let record = std::fs::read_to_string(stamp_path(out)).ok()?;
    let (stamp, rest) = split_record(&record);
    let mut stored = Stored {
        stamp: stamp.to_string(),
        depends_on: Vec::new(),
        skipped: Vec::new(),
    };
    for line in rest.lines() {
        match Dependency::parse(line) {
            Some(dependency) => stored.depends_on.push(dependency),
            None => stored.skipped.push(line.to_string()),
        }
    }
    out.is_file().then_some(stored)
}

/// The stamp and skips of the conversion cached at `out`, when there is one.
pub(super) fn stored(out: &Path) -> Option<(String, Vec<String>)> {
    read(out).map(|stored| (stored.stamp, stored.skipped))
}

/// What the conversion cached at `out` skipped, when it is stamped `stamp` and
/// every file it looked at still reads as it did; `None` when it must be
/// converted.
pub(super) fn current(out: &Path, stamp: &str) -> Option<Vec<String>> {
    let stored = read(out)?;
    let holds = stored.stamp == stamp && stored.depends_on.iter().all(Dependency::holds);
    holds.then_some(stored.skipped)
}

/// Warn of what the conversion of `source` stamped `stamp` skipped, as it ran.
pub(super) fn report(source: &Path, out: &Path, stamp: &str, skipped: &[String]) {
    first_time(out, stamp);
    warn(source, skipped);
}

/// Warn of what the conversion cached at `out` skipped, unless this process
/// already has.
pub(super) fn replay(source: &Path, out: &Path, stamp: &str, skipped: &[String]) {
    if first_time(out, stamp) {
        warn(source, skipped);
    }
}

/// Record (`out`, `stamp`) as warned of: true the first time.
fn first_time(out: &Path, stamp: &str) -> bool {
    let mut warned = WARNED.lock().unwrap_or_else(PoisonError::into_inner);
    warned.insert((out.to_path_buf(), stamp.to_string()))
}

fn warn(source: &Path, skipped: &[String]) {
    for skip in skipped {
        log::warn!("[Asset] {}: {skip}", source.display());
    }
}

/// Forget the warnings given for `out`, as a fresh process would.
#[cfg(test)]
pub(super) fn forget(out: &Path) {
    let mut warned = WARNED.lock().unwrap_or_else(PoisonError::into_inner);
    warned.retain(|(cached, _)| cached != out);
}
