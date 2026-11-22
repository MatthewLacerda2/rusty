use crate::layers::scan::{is_ident, live_lines};
use std::borrow::Cow;
use std::collections::BTreeSet;

pub struct CrateRef {
    pub line: usize,
    pub krate: String,
}

/// A file that declares `mod image;` or imports `super::image` means its own `image::`, not the crate.
pub fn crate_refs(content: &str, crates: &BTreeSet<String>) -> Vec<CrateRef> {
    let lines = live_lines(content);
    let uses = use_statements(&lines);
    let mut shadowed: BTreeSet<String> = lines.iter().filter_map(|(_, c)| mod_name(c)).collect();
    for (_, paths) in &uses {
        for (path, binding) in paths {
            if path.as_slice() != std::slice::from_ref(binding) {
                shadowed.insert(binding.clone());
            }
        }
    }
    let counts = |name: &str| crates.contains(name) && !shadowed.contains(name);
    let mut out = Vec::new();
    let in_use: BTreeSet<usize> = uses.iter().flat_map(|(r, _)| r.clone()).collect();
    for (range, paths) in &uses {
        for (path, _) in paths.iter().filter(|(p, _)| counts(&p[0])) {
            out.push(CrateRef {
                line: range.start,
                krate: path[0].clone(),
            });
        }
    }
    for (line, code) in lines.iter().filter(|(l, _)| !in_use.contains(l)) {
        let line = *line;
        for name in path_heads(code).into_iter().filter(|n| counts(n)) {
            out.push(CrateRef { line, krate: name });
        }
    }
    out.sort_by(|a, b| (a.line, &a.krate).cmp(&(b.line, &b.krate)));
    out.dedup_by(|a, b| (a.line, &a.krate) == (b.line, &b.krate));
    out
}

fn mod_name(code: &str) -> Option<String> {
    let t = code.trim_start();
    let t = t
        .strip_prefix("pub")
        .map_or(t, |r| r.trim_start_matches(|c| c != ' '));
    let rest = t.trim_start().strip_prefix("mod ")?;
    let end = rest.find(|c: char| !is_ident(c)).unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

type Paths = Vec<(Vec<String>, String)>;

fn use_statements(lines: &[(usize, Cow<str>)]) -> Vec<(std::ops::Range<usize>, Paths)> {
    let mut out = Vec::new();
    let mut open: Option<(usize, String)> = None;
    for (line, code) in lines {
        let line = *line;
        if open.is_none() {
            open = use_tree_start(code).map(|tree| (line, tree.to_string()));
        } else if let Some(o) = open.as_mut() {
            o.1.push(' ');
            o.1.push_str(code);
        }
        if let Some((start, text)) = open.take_if(|(_, t)| t.contains(';')) {
            let tree = text.split(';').next().unwrap_or("");
            out.push((start..line + 1, flatten(tree)));
        }
    }
    out
}

fn use_tree_start(code: &str) -> Option<&str> {
    let t = code.trim_start();
    let t = match t.strip_prefix("pub") {
        Some(r) if r.starts_with('(') => r.split_once(')')?.1.trim_start(),
        Some(r) => r.trim_start(),
        None => t,
    };
    t.strip_prefix("use ")
}

pub fn flatten(tree: &str) -> Paths {
    let tokens = tokenize(tree);
    let mut out = Vec::new();
    walk(&mut tokens.iter().peekable(), &[], &mut out);
    out
}

type Tokens<'a> = std::iter::Peekable<std::slice::Iter<'a, String>>;

fn walk(tokens: &mut Tokens, prefix: &[String], out: &mut Paths) {
    let mut path = prefix.to_vec();
    while let Some(tok) = tokens.next_if(|t| *t != "," && *t != "}") {
        match tok.as_str() {
            "::" => {}
            "{" => {
                group(tokens, &path, out);
                return;
            }
            "*" => return,
            "as" => {
                if let (false, Some(alias)) = (path.is_empty(), tokens.next()) {
                    out.push((path, alias.clone()));
                }
                return;
            }
            seg if seg == "self" && !path.is_empty() => {}
            seg => path.push(seg.to_string()),
        }
    }
    if let Some(last) = path.last().cloned() {
        out.push((path, last));
    }
}

fn group(tokens: &mut Tokens, prefix: &[String], out: &mut Paths) {
    while tokens.next_if(|t| *t == "}").is_none() {
        walk(tokens, prefix, out);
        if tokens.next_if(|t| *t == ",").is_none() {
            tokens.next();
            return;
        }
    }
}

fn tokenize(tree: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = tree.trim_start();
    while let Some(c) = rest.chars().next() {
        let len = if rest.starts_with("::") {
            2
        } else if is_ident(c) {
            rest.find(|c: char| !is_ident(c)).unwrap_or(rest.len())
        } else {
            c.len_utf8()
        };
        out.push(rest[..len].to_string());
        rest = rest[len..].trim_start();
    }
    out
}

fn path_heads(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (at, _) in code.match_indices("::") {
        let head = &code[..at];
        let start = head.rfind(|c: char| !is_ident(c)).map_or(0, |p| p + 1);
        let name = &head[start..];
        let before = head[..start].chars().next_back();
        if !name.is_empty() && before != Some(':') && !name.starts_with(char::is_numeric) {
            out.push(name.to_string());
        }
    }
    out
}
