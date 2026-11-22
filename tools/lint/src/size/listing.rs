use super::{skipped, walk, Lang};
use crate::repo::ls_files;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn committable(base: &Path, roots: &[&str]) -> Result<Vec<PathBuf>, String> {
    // `--cached` still lists a tracked file deleted from the working tree.
    let flags = ["--cached", "--others", "--exclude-standard"];
    Ok(ls_files(base, &flags, roots)?
        .into_iter()
        .map(|p| base.join(p))
        .filter(|p| p.is_file())
        .collect())
}

/// Every measured file on disk but what the root `.gitignore` anchors as a folder of its own
/// (`/project/`, `/out/`): what git would list, near enough, when git cannot.
pub(super) fn on_disk(root: &Path) -> Vec<PathBuf> {
    let ignored = ignored_roots(root);
    let wanted = |p: &Path| {
        let rel = p.strip_prefix(root).unwrap_or(p).to_string_lossy();
        Lang::of(p).is_some() || crate::comments::reads_prose(rel.trim_start_matches("./"))
    };
    let mut out = Vec::new();
    for entry in fs::read_dir(root).into_iter().flatten().flatten() {
        let (path, name) = (
            entry.path(),
            entry.file_name().to_string_lossy().into_owned(),
        );
        if !path.is_dir() {
            out.extend(Some(path).filter(|p| wanted(p)));
        } else if !skipped(&name) && !ignored.contains(&name) {
            walk(&path, &mut out, &wanted);
        }
    }
    out
}

/// The `/<name>/` rules: one anchored segment, no glob.
fn ignored_roots(root: &Path) -> Vec<String> {
    let rules = fs::read_to_string(root.join(".gitignore")).unwrap_or_default();
    rules
        .lines()
        .filter_map(|l| l.trim().strip_prefix('/')?.strip_suffix('/'))
        .filter(|name| !name.is_empty() && !name.contains(['/', '*', '?', '[', '!']))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
#[path = "listing_tests.rs"]
mod tests;
