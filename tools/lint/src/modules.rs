use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};

pub const CRATES: &str = "crates";

pub const CRATE_PREFIX: &str = "rusty_";

pub const NOT_MODULES: &[(&str, &str)] = &[(
    "test-support",
    "the in-crate tests' shared helpers, only ever a dev-dependency",
)];

pub fn crate_module(path: &str) -> Option<&str> {
    let rest = path.strip_prefix(CRATES)?.strip_prefix('/')?;
    let name = rest.split('/').next().filter(|n| !n.is_empty())?;
    (!NOT_MODULES.iter().any(|(n, _)| *n == name)).then_some(name)
}

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

pub fn dir(root: &Path, m: &str) -> PathBuf {
    root.join(CRATES).join(m).join("src")
}

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

pub fn all(root: &Path) -> Vec<String> {
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
