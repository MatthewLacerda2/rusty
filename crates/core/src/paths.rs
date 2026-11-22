//! Stored paths: relative to the project root and `/`-separated, so every machine reads
//! the same files.

use std::path::{Path, PathBuf};

/// Lexical: a symlinked spelling only matches itself ([`relativize`] resolves those).
/// Only `/` separates: a `\` is a filename character on macOS and Linux.
pub fn relative_to(path: &str, root: &str) -> Option<String> {
    let (path, root) = (components(path), components(root));
    if path.len() <= root.len() || root.is_empty() || path[..root.len()] != root[..] {
        return None;
    }
    let rest = &path[root.len()..];
    (!rest.contains(&"..")).then(|| rest.join("/"))
}

/// Falls back to canonical forms (macOS's `/var` is `/private/var`).
pub fn relativize(path: &str, root: &Path) -> String {
    if Path::new(path).is_relative() {
        return path.to_string();
    }
    let lexical = relative_to(path, &root.to_string_lossy());
    let canonical = || {
        let path = std::fs::canonicalize(path).ok()?;
        let root = std::fs::canonicalize(root).ok()?;
        relative_to(&path.to_string_lossy(), &root.to_string_lossy())
    };
    lexical
        .or_else(canonical)
        .unwrap_or_else(|| path.to_string())
}

/// The one rule for an input the project doesn't hold (`require`, a texture recipe's
/// image). An empty `root` is the working directory. Both sides are canonical.
pub fn outside_root(file: &Path, root: &Path) -> Option<PathBuf> {
    let real = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
    let root = if root.as_os_str().is_empty() {
        Path::new(".")
    } else {
        root
    };
    let inside = root.canonicalize().is_ok_and(|root| real.starts_with(root));
    (!inside).then_some(real)
}

pub fn shown(real: &Path) -> String {
    to_slash(&real.canonicalize().unwrap_or_else(|_| real.to_path_buf()))
}

pub fn to_slash(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// `path` as a stored asset path is kept, so one file is one key: trimmed and
/// without a leading `./` (` ./assets/a.cube` → `assets/a.cube`).
pub fn stored(path: &str) -> String {
    let mut slashed = to_slash(Path::new(path.trim()));
    while let Some(rest) = slashed.strip_prefix("./") {
        slashed = rest.trim_start_matches('/').to_string();
    }
    slashed
}

pub fn extension(path: impl AsRef<Path>) -> String {
    path.as_ref()
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// `ext` is lowercase, without the dot.
pub fn has_extension(path: impl AsRef<Path>, ext: &str) -> bool {
    extension(path) == ext
}

pub fn fold(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out: Vec<Component> = Vec::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => match out.last() {
                Some(Component::Normal(_)) => {
                    out.pop();
                }
                Some(Component::RootDir | Component::Prefix(_)) => {}
                _ => out.push(part),
            },
            _ => out.push(part),
        }
    }
    if out.is_empty() {
        return PathBuf::from(".");
    }
    out.iter().collect()
}

fn components(path: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = path.split('/').collect();
    let absolute = parts.first() == Some(&"");
    parts.retain(|p| !p.is_empty() && *p != ".");
    if absolute {
        parts.insert(0, "");
    }
    parts
}

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;
