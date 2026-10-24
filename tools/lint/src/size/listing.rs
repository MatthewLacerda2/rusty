//! Which files the full scan measures: the ones that can be committed (#1373).
//!
//! The scan once walked the disk, so whatever the engine seeds into a gitignored
//! folder (the UI kit's scripts and the default shader it copies into
//! `tests/fixtures/project/assets/` when the fixture project opens) failed the gate
//! locally, though CI never sees those files. Git answers instead: tracked files
//! plus untracked ones `.gitignore` doesn't exclude, so a new file is measured
//! before it is added and a seeded one never is.
//!
//! Only the size gate reads this list. The code gates (rules, layers, determinism,
//! direction, components, parity) keep walking the disk: they check what cargo
//! compiles, and an ignored `.rs` under a scan root is compiled all the same. The
//! `ignored` rule flags exactly that file, so it must see it.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The committable files under `roots` in the repository at `base`, joined onto
/// `base`, sorted; `None` when git can't list them (no `.git`, as in a tarball, or
/// no `git` at all), and the caller walks the disk instead.
pub(super) fn committable(base: &Path, roots: &[&str]) -> Option<Vec<PathBuf>> {
    let dir = if base.as_os_str().is_empty() {
        Path::new(".")
    } else {
        base
    };
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .arg("--")
        .args(roots)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let mut files: Vec<PathBuf> = String::from_utf8_lossy(&out.stdout)
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(|p| base.join(p))
        // `--cached` still lists a tracked file deleted from the working tree.
        .filter(|p| p.is_file())
        .collect();
    // A file with a merge conflict is listed once per stage.
    files.sort();
    files.dedup();
    Some(files)
}

#[cfg(test)]
#[path = "listing_tests.rs"]
mod tests;
