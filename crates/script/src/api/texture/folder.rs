//! crates/script/src/api/texture/folder.rs — the folder a `Texture` bake writes into (#1850).
//!
//! A bake to `assets/textures/<new set>/brick` makes the missing folders first, as
//! the lightmap bake makes its own (`dev::bake::lightmap::pages`). Only inside the
//! project: a target outside it is written as before, into a folder that must
//! already exist, so the bake never writes anywhere it couldn't before.

use std::path::{Component, Path};

use rusty_core::paths::to_slash;

/// Make the missing folders above `file`, when they lie inside `root`. A relative
/// `file` is read against `root` (the working directory, as the write itself does).
/// A path whose missing part climbs out (`..`) is left to the write to refuse.
pub(super) fn make_parents(file: &str, root: &Path) -> Result<(), String> {
    let Some(parent) = Path::new(file)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    else {
        return Ok(());
    };
    let parent = root.join(parent);
    if parent.is_dir() || !inside(&parent, root) {
        return Ok(());
    }
    std::fs::create_dir_all(&parent)
        .map_err(|e| format!("cannot make the folder {}: {e}", to_slash(&parent)))
}

/// True when `dir` (which may not exist yet) lies inside `root`: its nearest
/// existing ancestor is inside `root` once canonical, and no missing part below it
/// is `..`.
fn inside(dir: &Path, root: &Path) -> bool {
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    let mut existing = dir;
    while !existing.exists() {
        match (existing.components().next_back(), existing.parent()) {
            (Some(Component::Normal(_)), Some(up)) => existing = up,
            _ => return false,
        }
    }
    existing
        .canonicalize()
        .is_ok_and(|real| real.starts_with(&root))
}

#[cfg(test)]
#[path = "folder_tests.rs"]
mod tests;
