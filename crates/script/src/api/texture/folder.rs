use std::path::{Component, Path};

use rusty_core::paths::to_slash;

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
