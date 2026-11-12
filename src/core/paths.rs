//! src/core/paths.rs — paths relative to a workspace root (#782, #783).
//!
//! A scene names its scripts by paths relative to the project root (#829, the
//! working directory once `core::project::open` ran), written with `/`, so a scene
//! saved on one machine — or by one test run — names the same files on the next.
//! These helpers turn an absolute path under a root back into that form, and
//! [`to_slash`] / [`stored`] spell a path the way a stored file keeps it.

use std::path::{Path, PathBuf};

/// `path` relative to `root`, `/`-separated, when `path` lies under `root`; `None`
/// otherwise (a relative path, another tree, `root` itself, or a remainder with a
/// `..` that may climb back out). Purely lexical, comparing components exactly,
/// so a symlinked spelling only matches itself — [`relativize`] resolves those.
/// Only `/` separates: a `\` is a filename character on macOS and Linux.
pub fn relative_to(path: &str, root: &str) -> Option<String> {
    let (path, root) = (components(path), components(root));
    if path.len() <= root.len() || root.is_empty() || path[..root.len()] != root[..] {
        return None;
    }
    let rest = &path[root.len()..];
    (!rest.contains(&"..")).then(|| rest.join("/"))
}

/// `path` relative to `root` when it is absolute and lies under it, else `path`
/// unchanged. Tries the paths as given, then their canonical forms, which resolve
/// symlinks and `..` (macOS's `/var` is `/private/var`).
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

/// `file`'s real path when it lies outside `root`, `None` inside it: the one rule
/// for "an input the project doesn't hold" (#1062), shared by the Lua VM's
/// `require` / `dofile` and the texture generator's `image` files. An empty `root`
/// is the working directory. Both sides are canonical, so a symlink on one side
/// only can't hide the match; print the result with [`shown`].
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

/// `real` as a message prints it: canonical when it exists, `/`-separated.
pub fn shown(real: &Path) -> String {
    to_slash(&real.canonicalize().unwrap_or_else(|_| real.to_path_buf()))
}

/// `path` as the `/`-separated string scenes and the editor store (#939): the
/// platform's own spelling, a `\` kept as the filename character it is. Non-UTF-8
/// bytes are replaced lossily.
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

/// `path`'s extension, lowercased; empty when it has none (#994). The one place
/// a file's kind is read from its name, so `.LUA` and `.lua` are the same file kind
/// everywhere.
pub fn extension(path: impl AsRef<Path>) -> String {
    path.as_ref()
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// True when `path`'s extension is `ext` (given lowercase, without the dot),
/// compared case-insensitively.
pub fn has_extension(path: impl AsRef<Path>, ext: &str) -> bool {
    extension(path) == ext
}

/// `path` with every `.` dropped and every `..` folded into the name before it,
/// without reading the disk: `a/b/../c` → `a/c`. A `..` with no name before it
/// is kept on a relative path and dropped at a root (`/..` is `/`). A path that
/// folds to nothing is `.`.
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

/// The path's components, split on `/`, with empty or `.` components
/// dropped. A leading `/` leaves an empty first component, so absolute and
/// relative paths never match.
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
