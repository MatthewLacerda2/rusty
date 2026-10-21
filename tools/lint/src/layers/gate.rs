//! What test code is, for the layering lint and the rules alike (#1122, #1120): an
//! attribute that compiles its item only under test, and the files that only
//! compile under test because a gated `mod x;` declares them. One definition, so
//! `make layers` and `make rules` never disagree about a line.
//!
//! A gate is `#[cfg(test)]` or `#[cfg(all(…, test, …))]` (`test` anywhere among
//! `all`'s arguments, nested `all` included), however rustfmt wraps it. `any(…)`
//! and `not(…)` are never test-only: `any(test, x)` also compiles outside tests.

use super::extent::attributes;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Does an attribute leading the item at `lines[0]` gate it to test builds? An
/// attribute and its item may share the line: `#[cfg(test)] use a::B;`.
pub fn is_test_gate(lines: &[&str]) -> bool {
    attributes(lines).attrs.iter().any(|attr| gates(attr))
}

/// Is the attribute `attr` a test gate?
fn gates(attr: &str) -> bool {
    let attr: String = attr.chars().filter(|c| !c.is_whitespace()).collect();
    attr.strip_prefix("#[cfg(")
        .and_then(|rest| rest.strip_suffix(")]"))
        .is_some_and(test_only)
}

/// `#[path = "x.rs"]` → `x.rs`.
fn path_of(attr: &str) -> Option<&str> {
    let rest = attr
        .strip_prefix("#[path")?
        .trim_start()
        .strip_prefix('=')?;
    rest.trim_start().strip_prefix('"')?.split('"').next()
}

fn test_only(predicate: &str) -> bool {
    if predicate == "test" {
        return true;
    }
    let Some(args) = predicate
        .strip_prefix("all(")
        .and_then(|rest| rest.strip_suffix(')'))
    else {
        return false;
    };
    let (mut depth, mut start) = (0, 0);
    let mut parts = Vec::new();
    for (at, c) in args.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&args[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    parts.push(&args[start..]);
    parts.into_iter().any(test_only)
}

/// The files among `files` that only compile under `cfg(test)`: declared by a
/// test-gated `mod x;` (with or without `#[path]`), or declared (at any depth) by
/// such a file. `source` gives a file's text where the caller already read it; any
/// other file is read here. Paths come back [`normalise`]d.
pub fn test_only_files<'a>(
    files: &[PathBuf],
    source: impl Fn(&Path) -> Option<&'a str>,
) -> BTreeSet<PathBuf> {
    let mods = |file: &Path| match source(file) {
        Some(text) => declared_mods(file, text),
        None => declared_mods(file, &fs::read_to_string(file).unwrap_or_default()),
    };
    let mut test_only = BTreeSet::new();
    let mut frontier: Vec<PathBuf> = Vec::new();
    for file in files {
        for (child, gated) in mods(file) {
            if gated && test_only.insert(child.clone()) {
                frontier.push(child);
            }
        }
    }
    while let Some(file) = frontier.pop() {
        for (child, _) in mods(&file) {
            if test_only.insert(child.clone()) {
                frontier.push(child);
            }
        }
    }
    test_only
}

/// The out-of-line modules `content` (the text of `file`) declares, each with
/// whether its declaration is test-gated. Unresolvable declarations are skipped.
fn declared_mods(file: &Path, content: &str) -> Vec<(PathBuf, bool)> {
    let lines: Vec<&str> = content.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let item = attributes(&lines[i..]);
        if let Some(name) = outline_mod_name(item.code.trim()) {
            let path = item.attrs.iter().find_map(|a| path_of(a));
            if let Some(child) = resolve(file, name, path) {
                out.push((child, item.attrs.iter().any(|a| gates(a))));
            }
        }
        i += item.line + 1;
    }
    out
}

/// `mod x;` / `pub(crate) mod x;` → `x`; anything else → `None`.
fn outline_mod_name(line: &str) -> Option<&str> {
    let rest = line.strip_suffix(';')?;
    let at = rest.find("mod ")?;
    let head = &rest[..at];
    if !(head.is_empty() || head.trim_end().starts_with("pub")) {
        return None;
    }
    let name = rest[at + 4..].trim();
    name.chars()
        .all(|c| c.is_alphanumeric() || c == '_')
        .then_some(name)
}

fn resolve(file: &Path, name: &str, path_attr: Option<&str>) -> Option<PathBuf> {
    let dir = file.parent()?;
    if let Some(p) = path_attr {
        return Some(normalise(&dir.join(p)));
    }
    let stem = file.file_stem()?.to_str()?;
    let base = if matches!(stem, "mod" | "lib" | "main") {
        dir.to_path_buf()
    } else {
        dir.join(stem)
    };
    [
        base.join(format!("{name}.rs")),
        base.join(name).join("mod.rs"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

/// `path` with its `.` segments dropped and each `..` folding the segment before
/// it, so a `#[path = "./x.rs"]` names the same file the directory walk found
/// (#1136). Lexical: symlinks are not followed.
pub fn normalise(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir
                if matches!(out.components().next_back(), Some(Component::Normal(_))) =>
            {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
#[path = "gate_tests.rs"]
mod tests;
