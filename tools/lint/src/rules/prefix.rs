//! `prefix-group`: group by subfolder, not by filename prefix (CLAUDE.md, #934).
//!
//! Three or more sibling `.rs` files whose names start with the same word (`nav_`,
//! `probe-`) are a subfolder waiting to happen: `nav_agent.rs` → `nav/agent.rs`.
//! Files count by stem: `x.rs` and `x_tests.rs` are one, and a bare `nav.rs` counts
//! with the `nav_*` files beside it. The finding anchors on the first `mod` line
//! declaring the group, so a deliberate exception is an allow right above it.
//!
//! The ≲10-files-per-directory aim beside it is a printed signal, never a failure.

use super::finding::Finding;
use std::collections::BTreeMap;
use std::fs;

pub const RULE: &str = "prefix-group";
/// Siblings sharing a word before the rule fires.
const MIN_GROUP: usize = 3;
/// CLAUDE.md's "aim for ≲10 source files per module directory".
pub const AIM_FILES_PER_DIR: usize = 10;

/// Flat registries CLAUDE.md exempts: one file is one public unit, so the flat list
/// is the documentation. Exact directories, not their subfolders.
const EXEMPT: &[(&str, &str)] = &[
    ("src/api", "one file per Lua namespace"),
    (
        "src/components",
        "one file per first-class component, named for its `Entity` field",
    ),
];

/// A file's grouping stem: the name without `.rs` or a `_tests` suffix; `None` for
/// the files that own a folder rather than sit in it.
fn stem(file: &str) -> Option<&str> {
    let s = file.strip_suffix(".rs")?;
    if matches!(s, "mod" | "main" | "lib") {
        return None;
    }
    Some(s.strip_suffix("_tests").unwrap_or(s))
}

/// The word a stem is grouped under: up to its first `_` or `-`, or the whole stem.
fn word(stem: &str) -> &str {
    stem.split(['_', '-']).next().unwrap_or(stem)
}

/// Each word shared by [`MIN_GROUP`] or more stems among `files` (one directory's
/// file names), with its stems sorted.
pub fn groups<'a>(files: &[&'a str]) -> Vec<(&'a str, Vec<&'a str>)> {
    let mut by_word: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for s in files.iter().filter_map(|f| stem(f)) {
        let stems = by_word.entry(word(s)).or_default();
        if !stems.contains(&s) {
            stems.push(s);
        }
    }
    by_word
        .into_iter()
        .filter(|(_, stems)| stems.len() >= MIN_GROUP)
        .map(|(w, mut stems)| {
            stems.sort_unstable();
            (w, stems)
        })
        .collect()
}

/// The 1-based line of the first `mod` declaration of any of `files` in `owner`.
pub fn anchor(owner: &str, files: &[&str]) -> Option<usize> {
    let mods: Vec<&str> = files.iter().filter_map(|f| f.strip_suffix(".rs")).collect();
    owner
        .lines()
        .position(|l| {
            let l = l.trim();
            let l = l.strip_prefix("pub(crate) ").unwrap_or(l);
            let l = l.strip_prefix("pub ").unwrap_or(l);
            l.strip_prefix("mod ")
                .and_then(|m| m.strip_suffix(';'))
                .is_some_and(|m| mods.contains(&m))
        })
        .map(|i| i + 1)
}

/// The file declaring `dir`'s modules: its `mod.rs` / `main.rs` / `lib.rs`, or the
/// `<dir>.rs` beside it.
fn owner_of(dir: &str) -> Option<String> {
    ["mod.rs", "main.rs", "lib.rs"]
        .iter()
        .map(|f| format!("{dir}/{f}"))
        .chain(std::iter::once(format!("{dir}.rs")))
        .find(|p| fs::metadata(p).is_ok_and(|m| m.is_file()))
}

/// The findings for one directory, given its file names.
pub fn check_dir(dir: &str, files: &[&str]) -> Vec<Finding> {
    if EXEMPT.iter().any(|(d, _)| *d == dir) {
        return Vec::new();
    }
    let owner = owner_of(dir).map(|p| (fs::read_to_string(&p).unwrap_or_default(), p));
    groups(files)
        .into_iter()
        .map(|(w, stems)| {
            let members: Vec<&str> = files
                .iter()
                .copied()
                .filter(|f| stem(f).is_some_and(|s| stems.contains(&s)))
                .collect();
            let (path, line) = owner
                .as_ref()
                .and_then(|(src, p)| Some((p.clone(), anchor(src, &members)?)))
                .unwrap_or_else(|| (format!("{dir}/{}.rs", stems[0]), 1));
            Finding {
                path,
                line,
                rule: RULE,
                message: format!(
                    "{} files in {dir}/ share the prefix `{w}` ({}): move them into `{w}/` and drop the prefix",
                    stems.len(),
                    stems.join(", ")
                ),
            }
        })
        .collect()
}
