//! Source scan for the layering lint: which top-level modules each file names.
//!
//! Test code is exempt (#724): an inline `#[cfg(test)]` item is skipped, and a file
//! declared as `#[cfg(test)] mod x;` (with or without `#[path]`) is dropped together
//! with every file it declares in turn. Not a parser — a coarse line scan, like the
//! direction guard.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// One `crate::<module>` reference in non-test, non-comment code.
pub struct Ref {
    pub line: usize,
    pub module: String,
}

/// Every `.rs` file under `dir`, sorted so reports are stable.
pub fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk_into(dir, &mut out);
    out.sort();
    out
}

fn walk_into(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_into(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The files under `src` that only compile under `cfg(test)`: declared by a
/// test-gated `mod x;`, or declared (at any depth) by such a file.
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
    let (mut gated, mut path_attr) = (false, None::<String>);
    let mut out = Vec::new();
    for raw in content.lines() {
        let line = raw.trim();
        if line.starts_with("#[") {
            gated |= is_test_cfg(line);
            if let Some(p) = line.strip_prefix("#[path = \"") {
                path_attr = p.split('"').next().map(str::to_string);
            }
            continue;
        }
        if let Some(name) = outline_mod_name(line) {
            if let Some(child) = resolve(file, name, path_attr.as_deref()) {
                out.push((child, gated));
            }
        }
        gated = false;
        path_attr = None;
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

/// `#[cfg(test)]`, `#[cfg(all(test, …))]` — but not `#[cfg(not(test))]`.
pub fn is_test_cfg(attr: &str) -> bool {
    attr.starts_with("#[cfg(") && has_word(attr, "test") && !attr.contains("not(test)")
}

fn has_word(s: &str, word: &str) -> bool {
    s.match_indices(word).any(|(at, _)| {
        let before = s[..at].chars().next_back();
        let after = s[at + word.len()..].chars().next();
        !before.is_some_and(is_ident) && !after.is_some_and(is_ident)
    })
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The `crate::<module>` references in `content`, minus comments and inline
/// `#[cfg(test)]` items. `crate::{a, b::C}` yields both `a` and `b`.
pub fn refs(content: &str) -> Vec<Ref> {
    let mut out = Vec::new();
    let mut skip_item = false;
    let mut depth = 0i32;
    let mut group: Option<i32> = None;
    for (i, raw) in content.lines().enumerate() {
        let code = strip_comment(raw);
        let trimmed = code.trim();
        if !skip_item && is_test_cfg(trimmed) {
            skip_item = true;
        }
        if skip_item {
            depth += brace_delta(code);
            if depth <= 0 && trimmed.ends_with([';', '}', ',']) {
                skip_item = false;
                depth = 0;
            }
            continue;
        }
        for module in crate_refs(code, &mut group) {
            out.push(Ref {
                line: i + 1,
                module,
            });
        }
    }
    out
}

fn brace_delta(code: &str) -> i32 {
    code.chars()
        .map(|c| match c {
            '{' => 1,
            '}' => -1,
            _ => 0,
        })
        .sum()
}

/// Module names a line references. `group` carries an open `crate::{` across
/// lines: `Some(depth)` while inside it, so a multi-line import still counts.
fn crate_refs(code: &str, group: &mut Option<i32>) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = code;
    if let Some(d) = *group {
        if d == 1 {
            out.extend(leading_ident(rest));
        }
        rest = group_names(rest, group, &mut out);
    }
    while let Some(at) = find_crate(rest) {
        let after = &rest[at + "crate::".len()..];
        if let Some(inner) = after.strip_prefix('{') {
            *group = Some(1);
            out.extend(leading_ident(inner));
            rest = group_names(inner, group, &mut out);
        } else {
            out.extend(leading_ident(after));
            rest = after;
        }
    }
    out
}

/// Walk an open `crate::{ … }` group, collecting the first segment of each entry
/// at depth 1; returns what follows the group's close (or "" if still open).
fn group_names<'a>(s: &'a str, group: &mut Option<i32>, out: &mut Vec<String>) -> &'a str {
    let Some(mut d) = *group else { return s };
    for (at, c) in s.char_indices() {
        match c {
            '{' => d += 1,
            '}' if d == 1 => {
                *group = None;
                return &s[at + 1..];
            }
            '}' => d -= 1,
            ',' if d == 1 => out.extend(leading_ident(&s[at + 1..])),
            _ => {}
        }
    }
    *group = Some(d);
    ""
}

fn find_crate(s: &str) -> Option<usize> {
    s.match_indices("crate::")
        .find(|(at, _)| !s[..*at].chars().next_back().is_some_and(is_ident))
        .map(|(at, _)| at)
}

fn leading_ident(s: &str) -> Option<String> {
    let s = s.trim_start();
    let end = s.find(|c: char| !is_ident(c)).unwrap_or(s.len());
    (end > 0 && s[..end] != *"self").then(|| s[..end].to_string())
}

fn strip_comment(line: &str) -> &str {
    match line.find("//") {
        Some(idx) => &line[..idx],
        None => line,
    }
}
