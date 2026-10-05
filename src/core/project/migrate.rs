//! src/core/project/migrate.rs — carry a legacy `./project` forward, once (#829).
//!
//! Before projects could live anywhere, the project was the `project/` folder inside
//! the engine checkout and every stored path named it: `project/assets/scripts/bot.lua`,
//! `project/scenes/default.scene`. Opening such a folder as a project root:
//!
//! 1. moves each top-level content folder (`scenes/`, `prefabs/`, `materials/`, …)
//!    under `assets/`, and `storage.json` into `saved/`;
//! 2. rewrites every stored path in the project's text files (scenes, prefabs, JSON,
//!    animation graphs, Lua, the seed manifest) to the project-relative form:
//!    `project/assets/x` → `assets/x`, `project/scenes/x` → `assets/scenes/x`.
//!
//! A project is legacy when its seed manifest or build settings still name a
//! `project/` path; after one migration neither does, so it never runs again.

use std::path::Path;

use super::{ASSETS_DIR, CACHE_DIR, SAVED_DIR};

const LEGACY: &str = "project/";
/// Top-level folders that stay at the root: the skeleton's own, and the harness's
/// scenarios (dev-only drivers, not game content).
const ROOT_DIRS: &[&str] = &[ASSETS_DIR, CACHE_DIR, SAVED_DIR, "scenarios"];
/// The text files that store paths.
const TEXT_EXTENSIONS: &[&str] = &["scene", "prefab", "json", "animgraph", "lua", "lightmaps"];
const MARKERS: &[&str] = &[".seeded", "build_settings.json"];

/// Migrate `root` when it holds a legacy layout; returns one line per change.
pub(super) fn legacy_layout(root: &Path) -> Result<Vec<String>, String> {
    if !is_legacy(root) {
        return Ok(Vec::new());
    }
    let mut lines = vec![format!("migrating a legacy layout in {}", root.display())];
    lines.extend(move_content(root)?);
    let mut rewritten = 0;
    rewrite_tree(root, &mut rewritten)?;
    lines.push(format!("rewrote the stored paths in {rewritten} file(s)"));
    Ok(lines)
}

fn is_legacy(root: &Path) -> bool {
    MARKERS.iter().any(|name| {
        let text = std::fs::read_to_string(root.join(name)).unwrap_or_default();
        text.contains(&format!(" {LEGACY}")) || text.contains(&format!("\"{LEGACY}"))
    })
}

fn move_content(root: &Path) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    let entries = std::fs::read_dir(root).map_err(|e| format!("{}: {e}", root.display()))?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let skip = name.starts_with('.') || ROOT_DIRS.contains(&name.as_str());
        if skip || !entry.path().is_dir() {
            continue;
        }
        let to = root.join(ASSETS_DIR).join(&name);
        // An empty folder there (a skeleton made before the move) is no conflict.
        let _ = std::fs::remove_dir(&to);
        if to.exists() {
            lines.push(format!("kept {name}/: {ASSETS_DIR}/{name}/ already exists"));
            continue;
        }
        rename(&entry.path(), &to)?;
        lines.push(format!("moved {name}/ to {ASSETS_DIR}/{name}/"));
    }
    let (storage, saved) = (
        root.join("storage.json"),
        root.join(SAVED_DIR).join("storage.json"),
    );
    if storage.is_file() && !saved.exists() {
        rename(&storage, &saved)?;
        lines.push(format!("moved storage.json to {SAVED_DIR}/"));
    }
    Ok(lines)
}

fn rename(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::rename(from, to).map_err(|e| format!("{} → {}: {e}", from.display(), to.display()))
}

fn rewrite_tree(dir: &Path, rewritten: &mut usize) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for path in entries.flatten().map(|e| e.path()) {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if path.is_dir() {
            let skip = name.starts_with('.') || name == CACHE_DIR || name == SAVED_DIR;
            if !skip {
                rewrite_tree(&path, rewritten)?;
            }
            continue;
        }
        let ext = path.extension().unwrap_or_default().to_string_lossy();
        if !(MARKERS.contains(&name.as_ref()) || TEXT_EXTENSIONS.contains(&ext.as_ref())) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        if let Some(new) = rewrite(&text) {
            std::fs::write(&path, new).map_err(|e| format!("{}: {e}", path.display()))?;
            *rewritten += 1;
        }
    }
    Ok(())
}

/// `text` with every legacy path made project-relative; `None` when it names none.
/// A legacy path is `project/` right after a quote or a space (a JSON or Lua string,
/// a seed-manifest key), so prose that merely contains the word is left alone.
pub(super) fn rewrite(text: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut changed = false;
    while let Some(at) = rest.find(LEGACY) {
        out.push_str(&rest[..at]);
        let starts_path = matches!(out.as_bytes().last(), None | Some(b'"' | b'\'' | b' '));
        rest = &rest[at + LEGACY.len()..];
        if !starts_path {
            out.push_str(LEGACY);
            continue;
        }
        changed = true;
        let stays = ROOT_DIRS.iter().any(|d| rest.starts_with(&format!("{d}/")));
        if !stays {
            out.push_str(ASSETS_DIR);
            out.push('/');
        }
    }
    out.push_str(rest);
    changed.then_some(out)
}
