//! A cache's stamp file (`<cache>.glb.source`). Skips replay **once per process** per
//! cache and record: `import_file` runs per mesh entity, and replaying each would
//! flood the log.

use super::depends::Dependency;
use super::{Converted, Converter, Skip};
use rusty_core::collections::Set;
use rusty_core::hash;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};

/// Keyed by the whole record: another process's re-conversion keeps the stamp but may
/// skip differently.
static WARNED: LazyLock<Mutex<Set<(PathBuf, String)>>> = LazyLock::new(Mutex::default);

/// E.g. `blender 1 <digest> 5.0.1`.
pub(super) fn stamp(converter: &Converter, digest: u64, tool: Option<&str>) -> String {
    let base = format!("{} {} {digest:016x}", converter.name, converter.version);
    match tool {
        Some(tool) => format!("{base} {tool}\n"),
        None => format!("{base}\n"),
    }
}

fn untooled(stamp: &str) -> Vec<&str> {
    stamp.split_whitespace().take(3).collect()
}

pub(super) fn stamp_path(out: &Path) -> PathBuf {
    super::with_suffix(out, ".source")
}

const TEXTURES: &str = "textures ";

/// Digests the stamp and how each looked-at file reads now, so a re-conversion caused
/// only by a changed dependency gets its own folder and never rewrites a file the live
/// `.glb` reads.
pub(super) fn folder_tag(out: &Path, stamp: &str) -> String {
    let mut key = stamp.to_string();
    for dependency in stored(out).iter().flat_map(|s| &s.depends_on) {
        key.push_str(&dependency.now().line());
    }
    format!("{:016x}", hash::content(key.as_bytes()))
}

pub(super) fn tag_of(record: &str) -> Option<&str> {
    record.lines().find_map(|line| line.strip_prefix(TEXTURES))
}

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

pub(super) fn split_record(record: &str) -> (&str, &str) {
    let end = record.find('\n').map_or(record.len(), |i| i + 1);
    record.split_at(end)
}

pub(super) struct Stored {
    text: String,
    stamp: String,
    pub(super) depends_on: Vec<Dependency>,
    pub skipped: Vec<String>,
}

impl Stored {
    pub(super) fn written(stamp: &str, tag: &str, converted: &Converted) -> Self {
        Self {
            text: record(stamp, tag, converted),
            stamp: stamp.to_string(),
            depends_on: converted.depends_on.clone(),
            skipped: converted.skipped.iter().map(Skip::line).collect(),
        }
    }
}

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

pub(super) fn current(out: &Path, stamp: &str) -> Option<Stored> {
    let stored = stored(out)?;
    let holds = stored.stamp == stamp && stored.depends_on.iter().all(Dependency::holds);
    holds.then_some(stored)
}

pub(super) fn current_by_any_tool(out: &Path, stamp: &str) -> Option<Stored> {
    let stored = stored(out)?;
    let same = untooled(&stored.stamp) == untooled(stamp);
    (same && stored.depends_on.iter().all(Dependency::holds)).then_some(stored)
}

pub(super) fn report(source: &Path, out: &Path, stored: &Stored) {
    first_time(out, stored);
    warn(source, &stored.skipped);
}

pub(super) fn replay(source: &Path, out: &Path, stored: &Stored) {
    if first_time(out, stored) {
        warn(source, &stored.skipped);
    }
}

fn first_time(out: &Path, stored: &Stored) -> bool {
    let mut warned = WARNED.lock().unwrap_or_else(PoisonError::into_inner);
    warned.insert((out.to_path_buf(), stored.text.clone()))
}

fn warn(source: &Path, skipped: &[String]) {
    for skip in skipped {
        log::warn!("[Asset] {}: {skip}", source.display());
    }
}

#[cfg(test)]
pub(super) fn forget(out: &Path) {
    let mut warned = WARNED.lock().unwrap_or_else(PoisonError::into_inner);
    warned.retain(|(cached, _)| cached != out);
}
