//! src/core/paths.rs — paths relative to a workspace root (#782, #783).
//!
//! A scene names its scripts by paths relative to the project root (#829, the
//! working directory once `core::project::open` ran), written with `/` on every OS, so a scene saved on one
//! machine — or by one test run — names the same files on the next. These helpers
//! turn an absolute path under a root back into that form.

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
