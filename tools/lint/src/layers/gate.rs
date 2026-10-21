//! What test code is, for the layering lint and the rules alike (#1122, #1120): an
//! attribute that compiles its item only under test, and the files that only
//! compile under test because a gated `mod x;` declares them. One definition, so
//! `make layers` and `make rules` never disagree about a line.
//!
//! A gate is `#[cfg(test)]` or `#[cfg(all(…, test, …))]` (`test` anywhere among
//! `all`'s arguments, nested `all` included), however rustfmt wraps it. `any(…)`
//! and `not(…)` are never test-only: `any(test, x)` also compiles outside tests.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Does the attribute opening `lines[0]` gate its item to test builds? An
/// attribute and its item may share the line: `#[cfg(test)] use a::B;`.
pub fn is_test_gate(lines: &[&str]) -> bool {
    attribute(lines).is_some_and(|(attr, _)| gates(&attr))
}

/// Is the joined attribute `attr` a test gate?
fn gates(attr: &str) -> bool {
    attr.strip_prefix("#[cfg(")
        .and_then(|rest| rest.strip_suffix(")]"))
        .is_some_and(test_only)
}

/// The `#[…]` attribute opening `lines[0]`, its wrapped lines joined and
/// whitespace dropped, and how many lines it spans; `None` when `lines[0]` opens
/// no attribute or it never closes within a few lines.
fn attribute(lines: &[&str]) -> Option<(String, usize)> {
    if !lines.first()?.trim_start().starts_with("#[") {
        return None;
    }
    let (mut attr, mut depth) = (String::new(), 0);
    for (n, line) in lines.iter().take(12).enumerate() {
        for c in line.chars().filter(|c| !c.is_whitespace()) {
            attr.push(c);
            match c {
                '[' | '(' => depth += 1,
                ')' => depth -= 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some((attr, n + 1));
                    }
                }
                _ => {}
            }
        }
    }
    None
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
/// such a file.
pub fn test_only_files(files: &[PathBuf]) -> BTreeSet<PathBuf> {
    let mut test_only = BTreeSet::new();
    let mut frontier: Vec<PathBuf> = Vec::new();
    for file in files {
        for (child, gated) in declared_mods(file) {
            if gated && test_only.insert(child.clone()) {
                frontier.push(child);
            }
        }
    }
    while let Some(file) = frontier.pop() {
        for (child, _) in declared_mods(&file) {
            if test_only.insert(child.clone()) {
                frontier.push(child);
            }
        }
    }
    test_only
}

/// The out-of-line modules `file` declares, each with whether its declaration is
/// test-gated. Unresolvable declarations are skipped.
fn declared_mods(file: &Path) -> Vec<(PathBuf, bool)> {
    let Ok(content) = fs::read_to_string(file) else {
        return Vec::new();
    };
    let lines: Vec<&str> = content.lines().collect();
    let (mut gated, mut path_attr) = (false, None::<String>);
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        if let Some((attr, span)) = attribute(&lines[i..]) {
            gated |= gates(&attr);
            if let Some(p) = line.strip_prefix("#[path = \"") {
                path_attr = p.split('"').next().map(str::to_string);
            }
            i += span;
            continue;
        }
        if let Some(name) = outline_mod_name(line) {
            if let Some(child) = resolve(file, name, path_attr.as_deref()) {
                out.push((child, gated));
            }
        }
        gated = false;
        path_attr = None;
        i += 1;
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
        return Some(dir.join(p));
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

#[cfg(test)]
#[path = "gate_tests.rs"]
mod tests;
