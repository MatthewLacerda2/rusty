//! src/core/paths.rs — paths relative to a workspace root (#782, #783).
//!
//! A scene names its scripts by paths relative to the workspace root (the directory
//! holding `project/`), written with `/` on every OS, so a scene saved on one
//! machine — or by one test run — names the same files on the next. These helpers
//! turn an absolute path under a root back into that form.

use std::path::Path;

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
