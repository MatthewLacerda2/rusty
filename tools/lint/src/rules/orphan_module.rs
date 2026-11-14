//! `orphan-module`: every scanned `.rs` file is one cargo compiles.
//!
//! rustc builds what a crate root reaches through `mod` and nothing else. A file
//! no `mod` names is silently left out: no error, clippy and rustfmt skip it, and
//! its tests stop running while a count of `#[test]` lines still finds them. So
//! every scanned `.rs` file must be one of:
//! - a crate root: a target's `src_path` in `cargo metadata --no-deps` of each
//!   workspace (the root, and every scanned folder whose `Cargo.toml` holds its own
//!   `[workspace]`: `fuzz/`, `tools/lint/`), so a manifest's `path =` counts;
//! - reached from one by `mod`, resolved as rustc does (`module_tree.rs`),
//!   a `mod` under `#[cfg(...)]` included, since some build compiles it;
//! - named by `include!` or `include_str!`.
//!
//! A file meant to be built some other way (a script run with `rustc`) says why on
//! its first line: `// lint: allow(orphan-module) <why>`. A workspace cargo cannot
//! read is reported on its manifest, since nothing under it can be checked.

use super::finding::Finding;
use super::scan::Scan;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

#[path = "module_tree.rs"]
mod tree;
use tree::{normal, Reach};

pub const RULE: &str = "orphan-module";

/// The rule over the repository at the working directory.
pub fn run(scan: &Scan) -> Vec<Finding> {
    check(Path::new("."), &scan.files)
}

/// The rule over `files` (relative to `base`).
pub fn check(base: &Path, files: &[String]) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut roots = BTreeSet::new();
    for ws in workspaces(base, files) {
        match crate_roots(base, &ws) {
            Ok(found) => roots.extend(found),
            Err(why) => findings.push(Finding {
                path: format!("{ws}Cargo.toml"),
                line: 1,
                rule: RULE,
                message: format!(
                    "`cargo metadata` cannot read this workspace, so no file under it can \
                     be checked for a missing `mod`: {why}"
                ),
            }),
        }
    }
    let reached = reach(base, roots);
    let mut orphans: Vec<&String> = files.iter().filter(|f| !reached.contains(*f)).collect();
    orphans.sort();
    findings.extend(orphans.into_iter().map(|f| orphan(f)));
    findings
}

/// Each workspace's folder (`""` for the root, else `dir/`): the root when it has a
/// manifest, and every folder above a scanned file whose manifest has `[workspace]`.
fn workspaces(base: &Path, files: &[String]) -> Vec<String> {
    let mut dirs = BTreeSet::new();
    for f in files {
        let mut at = Path::new(f.as_str());
        while let Some(parent) = at.parent().filter(|p| !p.as_os_str().is_empty()) {
            dirs.insert(format!("{}/", normal(parent)));
            at = parent;
        }
    }
    let manifest = |dir: &str| fs::read_to_string(base.join(dir).join("Cargo.toml")).ok();
    let mut out: Vec<String> = manifest("").map(|_| String::new()).into_iter().collect();
    out.extend(
        dirs.into_iter()
            .filter(|d| manifest(d).is_some_and(|m| m.lines().any(|l| l.trim() == "[workspace]"))),
    );
    out
}

/// Every target's source file in the workspace at `dir`, relative to `base`.
fn crate_roots(base: &Path, dir: &str) -> Result<Vec<String>, String> {
    let manifest = base.join(dir).join("Cargo.toml");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    // lint: allow(subprocess) the crate roots are cargo's to say, `path =` targets included
    let out = Command::new(cargo)
        .args([
            "metadata",
            "--no-deps",
            "--offline",
            "--format-version",
            "1",
        ])
        .arg("--manifest-path")
        .arg(&manifest)
        .output()
        .map_err(|e| format!("cargo did not start: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(err.trim().lines().next().unwrap_or("it failed").to_string());
    }
    let at = fs::canonicalize(base).map_err(|e| e.to_string())?;
    let json = String::from_utf8_lossy(&out.stdout);
    Ok(src_paths(&json)
        .iter()
        .filter_map(|p| {
            let p = fs::canonicalize(p).unwrap_or_else(|_| p.into());
            Some(normal(p.strip_prefix(&at).ok()?))
        })
        .collect())
}

/// Every `"src_path"` value in `cargo metadata`'s JSON.
pub fn src_paths(json: &str) -> Vec<String> {
    const KEY: &str = "\"src_path\":\"";
    json.match_indices(KEY)
        .filter_map(|(i, _)| json_string(&json[i + KEY.len()..]))
        .collect()
}

/// A JSON string's value, from just past its opening quote.
fn json_string(s: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'u' => {
                    let hex: String = chars.by_ref().take(4).collect();
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                }
                'n' => out.push('\n'),
                't' => out.push('\t'),
                c => out.push(c),
            },
            c => out.push(c),
        }
    }
    None
}

/// Every file the crate roots reach, the roots included.
fn reach(base: &Path, roots: BTreeSet<String>) -> BTreeSet<String> {
    let (mut parsed, mut included) = (BTreeSet::new(), BTreeSet::new());
    let mut queue: Vec<(String, bool)> = roots.into_iter().map(|r| (r, true)).collect();
    while let Some((path, mod_rs)) = queue.pop() {
        if !parsed.insert(path.clone()) {
            continue;
        }
        let Ok(text) = fs::read_to_string(base.join(&path)) else {
            continue;
        };
        for r in tree::reached(&path, mod_rs, &text, |p| base.join(p).is_file()) {
            match r {
                Reach::Module { path, mod_rs } => queue.push((path, mod_rs)),
                // Read as text, not parsed: a module it also is still is, by its `mod`.
                Reach::Included(path) => {
                    included.insert(path);
                }
            }
        }
    }
    parsed.extend(included);
    parsed
}

fn orphan(path: &str) -> Finding {
    let p = Path::new(path);
    let (name, owned) = match p.file_name().and_then(|n| n.to_str()) {
        Some("mod.rs") => (p.parent(), p.parent().and_then(Path::parent)),
        _ => (Some(p), p.parent()),
    };
    let name = name
        .and_then(Path::file_stem)
        .map_or(String::new(), |n| n.to_string_lossy().into_owned());
    let owner = owned.map_or(String::new(), normal);
    Finding {
        path: path.to_string(),
        line: 1,
        rule: RULE,
        message: format!(
            "no `mod` declaration reaches this file, so cargo never compiles it and its \
             tests never run: declare `mod {name};` in the module that owns `{owner}/` \
             (its `mod.rs`, the `{owner}.rs` beside it, or the crate root there), or \
             delete the file if it is dead"
        ),
    }
}

#[cfg(test)]
#[path = "orphan_module_tests.rs"]
mod tests;
