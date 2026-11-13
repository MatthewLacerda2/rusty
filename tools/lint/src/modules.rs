//! Where a top-level module's code lives while the engine is split into crates
//! (`crates/CLAUDE.md`): `src/<m>/` until it moves, `crates/<m>/src/` after, with
//! `crates/<m>/src/lib.rs` in the place of `src/<m>/mod.rs`. Every pass that scopes
//! by module reads a file at its **module path**, the `src/<m>/…` spelling the
//! file has wherever it lives, so one rule or table row holds a module before and
//! after its move. A finding still names the file's real path.

use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};

/// The folder that holds the workspace's member crates.
pub const CRATES: &str = "crates";

/// How a module's crate is named in code: `rusty_<module>`.
pub const CRATE_PREFIX: &str = "rusty_";

/// Members that are no module of the layer table, each with why. Their code is
/// read at its real path, never as `src/<name>/`.
pub const NOT_MODULES: &[(&str, &str)] = &[(
    "test-support",
    "the in-crate tests' shared helpers, only ever a dev-dependency",
)];

/// The module a moved file belongs to: `<m>` for `crates/<m>/…`, unless `<m>` is
/// one of [`NOT_MODULES`].
pub fn crate_module(path: &str) -> Option<&str> {
    let rest = path.strip_prefix(CRATES)?.strip_prefix('/')?;
    let name = rest.split('/').next().filter(|n| !n.is_empty())?;
    (!NOT_MODULES.iter().any(|(n, _)| *n == name)).then_some(name)
}

/// `path` at its module path: `crates/<m>/src/lib.rs` → `src/<m>/mod.rs`,
/// `crates/<m>/src/<rest>` → `src/<m>/<rest>`, the folder `crates/<m>/src` →
/// `src/<m>`; any other path unchanged (a
/// module not yet moved, a crate's `build.rs` or manifest, a non-module member).
pub fn module_path(path: &str) -> Cow<'_, str> {
    let Some(m) = crate_module(path) else {
        return Cow::Borrowed(path);
    };
    let inner = &path[CRATES.len() + 1 + m.len()..];
    if inner == "/src" {
        return Cow::Owned(format!("src/{m}"));
    }
    match inner.strip_prefix("/src/") {
        Some("lib.rs") => Cow::Owned(format!("src/{m}/mod.rs")),
        Some(rest) => Cow::Owned(format!("src/{m}/{rest}")),
        None => Cow::Borrowed(path),
    }
}

/// The folder that holds module `m`'s code under `root`: `crates/<m>/src` once it
/// moved, else `src/<m>`.
pub fn dir(root: &Path, m: &str) -> PathBuf {
    let moved = root.join(CRATES).join(m).join("src");
    if moved.is_dir() {
        moved
    } else {
        root.join("src").join(m)
    }
}

/// Where the file or folder at module path `at` really is under `root`: the
/// inverse of [`module_path`] for a moved module, `root.join(at)` otherwise.
pub fn real(root: &Path, at: &str) -> PathBuf {
    let Some(rest) = at.strip_prefix("src/") else {
        return root.join(at);
    };
    let (m, inner) = rest.split_once('/').unwrap_or((rest, ""));
    let crate_src = root.join(CRATES).join(m).join("src");
    if !crate_src.is_dir() || NOT_MODULES.iter().any(|(n, _)| *n == m) {
        return root.join(at);
    }
    match inner {
        "" => crate_src,
        "mod.rs" => crate_src.join("lib.rs"),
        _ => crate_src.join(inner),
    }
}

/// The modules already moved under `root`: every member of `crates/` with a
/// `src/` folder but the [`NOT_MODULES`], sorted.
pub fn moved(root: &Path) -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir(root.join(CRATES))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join("src").is_dir())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|n| !NOT_MODULES.iter().any(|(m, _)| m == n))
        .collect();
    out.sort();
    out
}

/// Every member's manifest under `root` (`crates/*/Cargo.toml`), sorted.
pub fn member_manifests(root: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = fs::read_dir(root.join(CRATES))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path().join("Cargo.toml"))
        .filter(|p| p.is_file())
        .collect();
    out.sort();
    out
}

#[cfg(test)]
#[path = "modules_tests.rs"]
mod tests;
