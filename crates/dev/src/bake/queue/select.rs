//! A queue's scenes: a list, or every scene of the project but some.

use std::path::Path;

pub const SCENE_EXTENSION: &str = "scene";

#[derive(Clone, Debug, PartialEq)]
pub enum Selection {
    List(Vec<String>),
    All { except: Vec<String> },
}

/// `All` is sorted by path, so every run bakes one order. A listed path naming no scene
/// is kept: its entry fails.
pub fn resolve(root: &Path, selection: &Selection) -> Vec<String> {
    match selection {
        Selection::List(paths) => paths.iter().map(|p| normal(p)).collect(),
        Selection::All { except } => {
            let except: Vec<String> = except.iter().map(|p| normal(p)).collect();
            let mut found = Vec::new();
            walk(root, root, &mut found);
            found.sort();
            found.retain(|p| !except.contains(p));
            found
        }
    }
}

fn normal(path: &str) -> String {
    let path = path.replace('\\', "/");
    path.trim_start_matches("./").to_string()
}

fn walk(root: &Path, dir: &Path, found: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || (dir == root && SKIPPED.contains(&name.as_str())) {
            continue;
        }
        if path.is_dir() {
            walk(root, &path, found);
        } else if path.extension().is_some_and(|e| e == SCENE_EXTENSION) {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            found.push(normal(&relative.to_string_lossy()));
        }
    }
}

const SKIPPED: &[&str] = &["cache"];
