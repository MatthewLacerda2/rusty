use crate::code::is_ident;
use std::borrow::Cow;
use std::collections::BTreeMap;

/// The paths live code names, by 1-based line, resolved through the file's `use`s (read
/// whole), `rusty::m` as `rusty_m`. A `pub use` is no call; a glob imports nothing seen;
/// `crate`, `self` and `super` paths are dropped.
pub fn resolved(code: &[Cow<str>], test: &[bool]) -> Vec<(usize, String)> {
    let tested = |i: usize| test.get(i).copied().unwrap_or(false);
    let mut imports: BTreeMap<String, String> = BTreeMap::new();
    let mut found = Vec::new();
    let mut plain = Vec::new();
    let mut lines = code.iter().enumerate();
    while let Some((i, line)) = lines.next() {
        let Some((public, head)) = use_head(line) else {
            plain.push((i, line.as_ref()));
            continue;
        };
        let mut text = head.to_string();
        while !text.contains(';') {
            let Some((_, more)) = lines.next() else { break };
            text.push(' ');
            text.push_str(more);
        }
        if tested(i) {
            continue;
        }
        let tree = text.split(';').next().unwrap_or("");
        for (path, local) in expand(tree) {
            let path = canonical(&path);
            if !public {
                found.push((i + 1, path.clone()));
            }
            imports.insert(local, path);
        }
    }
    for (i, line) in plain.into_iter().filter(|(i, _)| !tested(*i)) {
        let paths = line.split(|c: char| !(is_ident(c) || c == ':'));
        for path in paths.filter_map(|p| segments(p.trim_matches(':'))) {
            let (first, rest) = path.split_once("::").unwrap_or((path, ""));
            let full = match imports.get(first) {
                Some(full) if rest.is_empty() => full.clone(),
                Some(full) => format!("{full}::{rest}"),
                None if matches!(first, "crate" | "self" | "super") => continue,
                None => canonical(path),
            };
            found.push((i + 1, full));
        }
    }
    found.sort();
    found
}

/// Whether a `use` line is public, and its text from the path on.
fn use_head(line: &str) -> Option<(bool, &str)> {
    let line = line.trim_start();
    if let Some(rest) = line.strip_prefix("use ") {
        return Some((false, rest));
    }
    let rest = line.strip_prefix("pub")?;
    let rest = match rest.strip_prefix('(') {
        Some(scoped) => scoped.split_once(')')?.1,
        None => rest,
    };
    Some((true, rest.trim_start().strip_prefix("use ")?))
}

/// `a::b` when every segment is an identifier.
fn segments(path: &str) -> Option<&str> {
    let ok = |s: &str| !s.is_empty() && s.chars().all(is_ident);
    (!path.is_empty() && path.split("::").all(ok)).then_some(path)
}

fn canonical(path: &str) -> String {
    let path = path.trim_start_matches("::");
    match path.strip_prefix("rusty::") {
        Some(rest) => format!("rusty_{rest}"),
        None => path.to_string(),
    }
}

/// Each path a `use` tree imports, with its local name.
pub fn expand(tree: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    expand_into("", tree, &mut out);
    out
}

fn expand_into(prefix: &str, tree: &str, out: &mut Vec<(String, String)>) {
    for part in top_level(tree) {
        let part = part.trim();
        if let Some((head, inner)) = part.split_once('{') {
            let inner = inner.trim_end().strip_suffix('}').unwrap_or(inner);
            let head: String = head.split_whitespace().collect();
            expand_into(&format!("{prefix}{head}"), inner, out);
            continue;
        }
        let (path, alias) = match part.split_once(" as ") {
            Some((path, alias)) => (path.trim(), Some(alias.trim())),
            None => (part, None),
        };
        let path: String = format!("{prefix}{path}").split_whitespace().collect();
        let path = path.strip_suffix("::self").unwrap_or(&path).to_string();
        let leaf = path.rsplit("::").next().unwrap_or("").to_string();
        let local = alias.map_or(leaf, str::to_string);
        if segments(path.trim_start_matches("::")).is_some() && local != "_" {
            out.push((path, local));
        }
    }
}

/// `tree` split at the commas outside its braces.
fn top_level(tree: &str) -> Vec<&str> {
    let mut depth = 0i32;
    let mut parts = Vec::new();
    let mut rest = tree;
    let mut taken = 0;
    for (at, c) in tree.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            ',' if depth == 0 => {
                let (part, after) = rest.split_at(at - taken);
                parts.push(part);
                rest = &after[1..];
                taken = at + 1;
            }
            _ => {}
        }
    }
    parts.push(rest);
    parts.retain(|p| !p.trim().is_empty());
    parts
}

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;
