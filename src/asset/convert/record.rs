//! A cache's stamp file (`<cache>.glb.source`): the stamp that says which
//! conversion the cache is, then one line per part it skipped (#1172), and the
//! warnings those skips raise.
//!
//! A conversion warns of its skips as it runs. A cache hit replays them **once per
//! process** per cache and stamp: `import_file` runs once per mesh entity on a
//! scene load, a `Scene.Instantiate` and a prefab propagation, and replaying on
//! each would flood the log. A model loaded in a fresh process still says what it
//! is missing, as Unity shows import warnings on every reimport.

use super::{Converted, Converter};
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

/// What a stamp file holds: the stamp, then one line per skip.
pub(super) fn record(stamp: &str, converted: &Converted) -> String {
    let mut record = stamp.to_string();
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

/// The stamp and skips of the conversion cached at `out`, when there is one.
pub(super) fn stored(out: &Path) -> Option<(String, Vec<String>)> {
    let record = std::fs::read_to_string(stamp_path(out)).ok()?;
    let (stamp, skipped) = split_record(&record);
    let skipped = skipped.lines().map(str::to_string).collect();
    out.is_file().then(|| (stamp.to_string(), skipped))
}

/// What the conversion cached at `out` skipped, when it is stamped `stamp`;
/// `None` when it must be converted.
pub(super) fn current(out: &Path, stamp: &str) -> Option<Vec<String>> {
    stored(out).and_then(|(found, skipped)| (found == stamp).then_some(skipped))
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
