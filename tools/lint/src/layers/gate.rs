use super::extent::attributes;
use crate::code::{strip_comment, Open};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub fn is_test_gate(lines: &[&str]) -> bool {
    attributes(lines).attrs.iter().any(|attr| gates(attr))
}

fn gates(attr: &str) -> bool {
    let attr: String = attr.chars().filter(|c| !c.is_whitespace()).collect();
    attr.strip_prefix("#[cfg(")
        .and_then(|rest| rest.strip_suffix(")]"))
        .is_some_and(test_only)
}

fn path_of(attr: &str) -> Option<&str> {
    let rest = attr
        .strip_prefix("#[path")?
        .trim_start()
        .strip_prefix('=')?;
    rest.trim_start().strip_prefix('"')?.split('"').next()
}

/// `any(..)` and `not(..)` are never test-only: `any(test, x)` also compiles outside tests.
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

fn declared_mods(file: &Path, content: &str) -> Vec<(PathBuf, bool)> {
    let lines: Vec<&str> = content.lines().collect();
    let mut out = Vec::new();
    let (mut i, mut gated, mut path) = (0, false, None::<String>);
    while i < lines.len() {
        let item = attributes(&lines[i..]);
        i += item.line + 1;
        gated |= item.attrs.iter().any(|a| gates(a));
        path = path.or_else(|| {
            item.attrs
                .iter()
                .find_map(|a| path_of(a))
                .map(str::to_owned)
        });
        let code = strip_comment(item.code, &mut Open::None);
        let code = code.trim();
        if code.is_empty() {
            continue;
        }
        if let Some(child) = outline_mod_name(code).and_then(|n| resolve(file, n, path.as_deref()))
        {
            out.push((child, gated));
        }
        (gated, path) = (false, None);
    }
    out
}

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
