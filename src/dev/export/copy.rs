//! The copy half of an export: a project's files into the output, in sorted order,
//! convertible sources left out.

use std::path::Path;

use crate::asset::convert::converter_for;
use crate::core::paths;

/// Copy the file `rel` of the project at `root` to the same place under `out`,
/// and record it in `written`.
pub(super) fn file(
    root: &Path,
    rel: &Path,
    out: &Path,
    written: &mut Vec<String>,
) -> Result<(), String> {
    let to = out.join(rel);
    if let Some(dir) = to.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let from = root.join(rel);
    std::fs::copy(&from, &to).map_err(|e| format!("copying {}: {e}", from.display()))?;
    written.push(paths::to_slash(rel));
    Ok(())
}

/// Copy every file under the folder `rel` of the project at `root` but the
/// sources a converter reads, visiting each folder's entries by name. A symlinked
/// folder is not followed, so a link back up the tree never loops; a symlinked
/// file is copied as the file it names.
pub(super) fn tree(
    root: &Path,
    rel: &Path,
    out: &Path,
    written: &mut Vec<String>,
) -> Result<(), String> {
    let dir = root.join(rel);
    let read = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut names = read
        .map(|entry| entry.map(|e| e.file_name()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    names.sort();
    for name in names {
        let rel = rel.join(name);
        let path = root.join(&rel);
        let kind = std::fs::symlink_metadata(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .file_type();
        if kind.is_dir() {
            tree(root, &rel, out, written)?;
        } else if converter_for(&rel).is_none() && path.is_file() {
            file(root, &rel, out, written)?;
        }
    }
    Ok(())
}
