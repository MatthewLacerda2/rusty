//! Step 5 of the swap: what earlier conversions of one cache left beside it.

use super::super::record::tag_of;
use super::super::stamp_path;
use super::{textures_name, STAGING, TEXTURES};
use crate::core::fs;
use std::path::{Path, PathBuf};

/// Remove what earlier conversions of `name` left in `dir`: every textures folder
/// but the one the stamp on disk names, staging folders and stamp temp files.
///
/// The stamp is read again before each folder goes, since another process may
/// have swapped its own conversion in meanwhile: the folder kept is always the
/// one the current `.glb` references, never the one this call staged. With no
/// stamp, a swap is between steps 3 and 4, so no textures folder is touched. A
/// staging folder still in use belongs to a concurrent conversion of the same
/// source: its converter then fails to write, and its `refresh` finds this
/// conversion in place or fails that one import.
pub(super) fn clear(dir: &Path, name: &str) {
    let stamp = stamp_path(&dir.join(name));
    let current = || {
        let record = std::fs::read_to_string(&stamp).ok()?;
        tag_of(&record).map(|tag| textures_name(name, tag))
    };
    for old in textures_dirs(&dir.join(name)) {
        let stale = current().is_some_and(|keep| old.file_name() != Some(keep.as_ref()));
        if stale {
            let _ = std::fs::remove_dir_all(old);
        }
    }
    let staging_of = format!(".{name}");
    let stamp_of = format!("{name}.source");
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let entry_name = entry.file_name().to_string_lossy().into_owned();
        if fs::is_unique_sibling(&entry_name, &staging_of, STAGING) {
            let _ = std::fs::remove_dir_all(entry.path());
        } else if fs::is_unique_sibling(&entry_name, &stamp_of, fs::TEMP_SUFFIX) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// The textures folders beside the cached glTF at `out`, current or left by an
/// earlier conversion: `<out>.<tag>.textures/`, and the untagged
/// `<out>.textures/` of converters before #1129.
pub fn textures_dirs(out: &Path) -> Vec<PathBuf> {
    let (dir, name) = split(out);
    std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| is_textures_of(&e.file_name().to_string_lossy(), &name))
        .map(|e| e.path())
        .collect()
}

fn is_textures_of(entry: &str, name: &str) -> bool {
    let Some(middle) = entry
        .strip_prefix(name)
        .and_then(|rest| rest.strip_suffix(TEXTURES))
    else {
        return false;
    };
    let tag = middle.strip_prefix('.').and_then(|m| m.strip_suffix('.'));
    middle == "." || tag.is_some_and(|t| t.len() == 16 && t.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// `out`'s folder (`.` for a bare file name) and file name: exact, since a
/// project's source path is UTF-8 (`project_relative`).
pub(super) fn split(out: &Path) -> (PathBuf, String) {
    let dir = match out.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let name = out
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    (dir, name)
}
