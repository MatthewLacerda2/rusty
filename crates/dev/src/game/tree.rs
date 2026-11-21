use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub(super) const MARKER: &str = ".test-game";

pub(super) fn clear(out: &Path) -> Result<(), String> {
    let shown = |e: std::io::Error| format!("{}: {e}", out.display());
    if out.join(MARKER).is_file() {
        std::fs::remove_dir_all(out).map_err(shown)?;
    } else if out.read_dir().is_ok_and(|mut d| d.next().is_some()) {
        return Err(format!(
            "{} is not empty and is no generated game (it has no {MARKER}): refusing to clear it",
            out.display()
        ));
    }
    std::fs::create_dir_all(out).map_err(shown)
}

pub(super) fn beside(out: &Path, suffix: &str) -> Result<PathBuf, String> {
    let name = out
        .file_name()
        .ok_or_else(|| format!("{} has no folder name to set a copy beside", out.display()))?;
    let mut aside = name.to_os_string();
    aside.push(format!(".{suffix}"));
    Ok(out.with_file_name(aside))
}

pub(super) fn remove(dir: &Path) -> Result<(), String> {
    let shown = |e: std::io::Error| format!("{}: {e}", dir.display());
    match std::fs::symlink_metadata(dir) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(shown(e)),
        Ok(meta) if meta.is_dir() && dir.join(MARKER).is_file() => {
            std::fs::remove_dir_all(dir).map_err(shown)
        }
        Ok(meta) if meta.is_dir() && dir.read_dir().is_ok_and(|mut d| d.next().is_none()) => {
            std::fs::remove_dir(dir).map_err(shown)
        }
        Ok(_) => Err(format!(
            "{} is in the way and is no generated game (it has no {MARKER}): refusing to delete it",
            dir.display()
        )),
    }
}

pub(super) fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
    for (rel, path) in files(from)? {
        let dest = to.join(&rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::copy(&path, &dest).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

pub(super) fn differences(a: &Path, b: &Path) -> Result<Vec<PathBuf>, String> {
    let (a, b) = (files(a)?, files(b)?);
    let mut differ: Vec<PathBuf> = a
        .iter()
        .filter(|(rel, path)| b.get(*rel).is_none_or(|other| !same_bytes(path, other)))
        .map(|(rel, _)| rel.clone())
        .collect();
    differ.extend(b.keys().filter(|rel| !a.contains_key(*rel)).cloned());
    differ.sort();
    Ok(differ)
}

fn same_bytes(a: &Path, b: &Path) -> bool {
    matches!((std::fs::read(a), std::fs::read(b)), (Ok(x), Ok(y)) if x == y)
}

fn files(root: &Path) -> Result<BTreeMap<PathBuf, PathBuf>, String> {
    let mut found = BTreeMap::new();
    if !root.is_dir() {
        return Ok(found);
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for entry in entries {
            let path = entry.map_err(|e| format!("{}: {e}", dir.display()))?.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(rel) = path.strip_prefix(root) {
                found.insert(rel.to_path_buf(), path.clone());
            }
        }
    }
    Ok(found)
}

#[cfg(test)]
#[path = "tree_tests.rs"]
mod tests;
