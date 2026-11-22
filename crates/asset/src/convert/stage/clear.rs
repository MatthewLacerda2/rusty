//! Step 5 of the swap.

use super::super::record::tag_of;
use super::super::stamp_path;
use super::{textures_name, STAGING, TEXTURES};
use rusty_core::fs;
use std::path::{Path, PathBuf};

/// The stamp is re-read before each folder goes (another process may have swapped in):
/// the folder kept is the one the current `.glb` references. With no stamp a swap is
/// between steps 3 and 4, so no textures folder is touched. A staging folder still in
/// use belongs to a concurrent conversion, whose converter then fails to write.
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

/// Current or left by an earlier conversion: `<out>.<tag>.textures/`.
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

/// A cached glTF's name ends `.glb`, which tells the tagged and untagged forms apart.
pub fn textures_owner(entry: &str) -> Option<&str> {
    let rest = entry.strip_suffix(TEXTURES)?.strip_suffix('.')?;
    let untagged = rest.ends_with(".glb").then_some(rest);
    let tagged = rest.rsplit_once('.').filter(|(name, tag)| {
        name.ends_with(".glb") && tag.len() == 16 && tag.bytes().all(|b| b.is_ascii_hexdigit())
    });
    tagged.map(|(name, _)| name).or(untagged)
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

/// `out`'s folder (`.` for a bare file name) and file name.
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
