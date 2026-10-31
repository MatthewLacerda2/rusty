//! A cache's stamp file (`<cache>.glb.source`): the stamp that says which
//! conversion the cache is, the tag of its textures folder ([`folder_tag`]), then
//! one line per external file it looked at (#1189), then one line per part it
//! skipped (#1172), and the warnings those skips raise.
//!
//! The files looked at are [`Dependency`] lines (`depends`); the cache is current
//! only while each still reads the same.
//!
//! A conversion warns of its skips as it runs. A cache hit replays them **once per
//! process** per cache and record (stamp, files and skips): `import_file` runs
//! once per mesh entity on a scene load, a `Scene.Instantiate` and a prefab
//! propagation, and replaying on each would flood the log. A model loaded in a fresh process still says what it
//! is missing, as Unity shows import warnings on every reimport.

use super::depends::Dependency;
use super::{Converted, Converter, Skip};
use crate::core::collections::Set;
use crate::core::hash;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};

/// The (cache, record) pairs whose skips this process has warned of: the whole
/// record, since a re-conversion another process made for a changed file keeps
/// the stamp but may skip differently.
static WARNED: LazyLock<Mutex<Set<(PathBuf, String)>>> = LazyLock::new(Mutex::default);

/// What the stamp records: the converter, its version, the source's digest, and
/// the version of the program the converter runs (`blender 1 <digest> 5.0.1`).
pub(super) fn stamp(converter: &Converter, digest: u64, tool: Option<&str>) -> String {
    let base = format!("{} {} {digest:016x}", converter.name, converter.version);
    match tool {
        Some(tool) => format!("{base} {tool}\n"),
        None => format!("{base}\n"),
    }
}

/// `stamp` without the version of the program that made it.
fn untooled(stamp: &str) -> Vec<&str> {
    stamp.split_whitespace().take(3).collect()
}

pub(super) fn stamp_path(out: &Path) -> PathBuf {
    super::with_suffix(out, ".source")
}

/// What starts the line naming the conversion's textures folder by its tag.
const TEXTURES: &str = "textures ";

/// The tag of the textures folder a conversion stamped `stamp` writes beside the
/// cache at `out`: a digest of the stamp and of how each file the cache's current
/// conversion looked at reads now. A conversion made again only because one of
/// those changed (a `.blend`'s linked library, whose packed images it extracts)
/// gets a folder of its own, so it never rewrites a file the live `.glb` reads
/// (#1072). Only a conversion whose stamp file was lost, or whose swap failed,
/// meets its own folder again (`stage::move_in`).
pub(super) fn folder_tag(out: &Path, stamp: &str) -> String {
    let mut key = stamp.to_string();
    for dependency in stored(out).iter().flat_map(|s| &s.depends_on) {
        key.push_str(&dependency.now().line());
    }
    format!("{:016x}", hash::content(key.as_bytes()))
}

/// The textures folder tag the stamp file `record` names.
pub(super) fn tag_of(record: &str) -> Option<&str> {
    record.lines().find_map(|line| line.strip_prefix(TEXTURES))
}

/// What a stamp file holds: the stamp, its textures folder's `tag`, then one line
/// per look at a file (each once: a file looked at twice and found the same is
/// one line), then one line per skip.
pub(super) fn record(stamp: &str, tag: &str, converted: &Converted) -> String {
    let mut record = format!("{stamp}{TEXTURES}{tag}\n");
    let mut lines = Set::default();
    for line in converted.depends_on.iter().map(Dependency::line) {
        if lines.insert(line.clone()) {
            record.push_str(&line);
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

/// A conversion's stamp file, read back or as it was written.
pub(super) struct Stored {
    /// The whole stamp file.
    text: String,
    stamp: String,
    pub(super) depends_on: Vec<Dependency>,
    pub skipped: Vec<String>,
}

impl Stored {
    /// What the conversion stamped `stamp` that reported `converted` writes.
    pub(super) fn written(stamp: &str, tag: &str, converted: &Converted) -> Self {
        Self {
            text: record(stamp, tag, converted),
            stamp: stamp.to_string(),
            depends_on: converted.depends_on.clone(),
            skipped: converted.skipped.iter().map(Skip::line).collect(),
        }
    }
}

/// The conversion cached at `out`, when there is one and its stamp file parses.
pub(super) fn stored(out: &Path) -> Option<Stored> {
    let text = std::fs::read_to_string(stamp_path(out)).ok()?;
    let (stamp, rest) = split_record(&text);
    let (mut depends_on, mut skipped) = (Vec::new(), Vec::new());
    for line in rest.lines().filter(|l| !l.starts_with(TEXTURES)) {
        match Dependency::parse(line) {
            Some(dependency) => depends_on.push(dependency?),
            None => skipped.push(line.to_string()),
        }
    }
    let stamp = stamp.to_string();
    out.is_file().then_some(Stored {
        text,
        stamp,
        depends_on,
        skipped,
    })
}

/// The conversion cached at `out`, when it is stamped `stamp` and every file it
/// looked at still reads as it did; `None` when it must be converted.
pub(super) fn current(out: &Path, stamp: &str) -> Option<Stored> {
    let stored = stored(out)?;
    let holds = stored.stamp == stamp && stored.depends_on.iter().all(Dependency::holds);
    holds.then_some(stored)
}

/// [`current`], made by any version of the converter's program: what stands in
/// while that program cannot be found.
pub(super) fn current_by_any_tool(out: &Path, stamp: &str) -> Option<Stored> {
    let stored = stored(out)?;
    let same = untooled(&stored.stamp) == untooled(stamp);
    (same && stored.depends_on.iter().all(Dependency::holds)).then_some(stored)
}

/// Warn of what the conversion of `source` that wrote `stored` skipped, as it ran.
pub(super) fn report(source: &Path, out: &Path, stored: &Stored) {
    first_time(out, stored);
    warn(source, &stored.skipped);
}

/// Warn of what the conversion cached at `out` skipped, unless this process
/// already has.
pub(super) fn replay(source: &Path, out: &Path, stored: &Stored) {
    if first_time(out, stored) {
        warn(source, &stored.skipped);
    }
}

/// Record (`out`, `stored`'s record) as warned of: true the first time.
fn first_time(out: &Path, stored: &Stored) -> bool {
    let mut warned = WARNED.lock().unwrap_or_else(PoisonError::into_inner);
    warned.insert((out.to_path_buf(), stored.text.clone()))
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
