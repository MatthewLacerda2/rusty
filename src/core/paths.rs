//! src/core/paths.rs — paths relative to a workspace root (#782, #783).
//!
//! A scene names its scripts by paths relative to the project root (#829, the
//! working directory once `core::project::open` ran), written with `/` on every OS, so a scene saved on one
//! machine — or by one test run — names the same files on the next. These helpers
//! turn an absolute path under a root back into that form, and [`native`] turns
//! such a path (or a `/`-spelled constant like `IMPORT_CACHE_DIR`) into the
//! platform's own (#1371).

use std::path::{Path, PathBuf};

/// `path` relative to `root`, `/`-separated, when `path` lies under `root`; `None`
/// otherwise (a relative path, another tree, `root` itself, or a remainder with a
/// `..` that may climb back out). Purely lexical, and it reads both `/` and `\` as
/// separators, so a Windows path parses on any OS. A drive letter compares
/// case-insensitively; every other component exactly, so a short name (`RUNNER~1`)
/// only matches itself — [`relativize`] resolves those.
pub fn relative_to(path: &str, root: &str) -> Option<String> {
    let (path, root) = (components(path), components(root));
    if path.len() <= root.len() || root.is_empty() {
        return None;
    }
    let same = |(i, (a, b)): (usize, (&&str, &&str))| {
        a == b || (i == 0 && a.ends_with(':') && a.eq_ignore_ascii_case(b))
    };
    if !path.iter().zip(&root).enumerate().all(same) {
        return None;
    }
    let rest = &path[root.len()..];
    (!rest.contains(&"..")).then(|| rest.join("/"))
}

/// `path` relative to `root` when it is absolute and lies under it, else `path`
/// unchanged. Tries the paths as given, then their canonical forms, which resolve
/// symlinks, `..` and Windows short names (`C:\Users\RUNNER~1` and
/// `C:\Users\runneradmin` name one directory).
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

/// [`std::fs::canonicalize`] without Windows' verbatim prefix (#904). Windows
/// returns `\\?\D:\...`, which it never normalises, so a `/` joined onto it is a
/// literal character and the read fails; macOS and Linux keep their symlink
/// resolution unchanged.
pub fn canonicalize(path: &Path) -> std::io::Result<PathBuf> {
    let canonical = std::fs::canonicalize(path)?;
    Ok(match canonical.to_str() {
        Some(s) => PathBuf::from(strip_verbatim(s)),
        None => canonical,
    })
}

/// `slashed`, a `/`-separated path, joined one part at a time: the platform's own
/// spelling (#1332, #1371). `Path::join("a/b")` keeps the `/`, so on Windows
/// `root.join("a/b")` is `root\a/b`; `root.join(native("a/b"))` is `root\a\b`,
/// the one spelling the engine writes everywhere and its tests compare against.
/// A leading `/` stays the root, as `Path::new("/")` is; empty parts drop.
pub fn native(slashed: &str) -> PathBuf {
    let (start, rest) = match slashed.strip_prefix('/') {
        Some(rest) => (PathBuf::from("/"), rest),
        None => (PathBuf::new(), slashed),
    };
    rest.split('/')
        .filter(|part| !part.is_empty())
        .fold(start, |path, part| path.join(part))
}

/// [`native`] as the string the engine writes into a file or a log line.
pub fn native_string(slashed: &str) -> String {
    native(slashed).to_string_lossy().into_owned()
}

/// `path` as a `/`-separated string, the form scenes and the editor store (#939):
/// every `\` becomes `/`, whatever the OS. Non-UTF-8 bytes are replaced lossily.
pub fn to_slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
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

/// `path` without a verbatim prefix: `\\?\C:\x` → `C:\x`, `\\?\UNC\host\share`
/// → `\\host\share`. Any other path, `/`-rooted ones included, comes back as is.
pub fn strip_verbatim(path: &str) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    path.strip_prefix(r"\\?\").unwrap_or(path).to_string()
}

/// The path's components, with Windows' verbatim prefix (`\\?\`, which
/// `canonicalize` adds) and empty or `.` components dropped. A leading `/` leaves
/// an empty first component, so absolute and relative paths never match.
fn components(path: &str) -> Vec<&str> {
    let path = path.strip_prefix(r"\\?\").unwrap_or(path);
    let mut parts: Vec<&str> = path.split(['/', '\\']).collect();
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
