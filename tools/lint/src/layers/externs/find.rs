//! Which external crates a file names: the scan behind the crate half of the table
//! (#938). Not a parser, like the `crate::` scan beside it, but it knows the one
//! thing a line scan gets wrong: a local item that shares a crate's name. A file
//! that declares `mod image;` or imports `super::image` means its own `image::` when
//! it writes one, so that name is not counted there. Glob imports (`use super::*`)
//! are not followed.

use crate::layers::scan::{is_ident, live_lines};
use std::borrow::Cow;
use std::collections::BTreeSet;

/// One external crate named at `line`.
pub struct CrateRef {
    pub line: usize,
    pub krate: String,
}

/// Every reference to one of `crates` in the live code of `content`: the first
/// segment of a `use` path, or a `name::` path anywhere else.
pub fn crate_refs(content: &str, crates: &BTreeSet<String>) -> Vec<CrateRef> {
    let lines = live_lines(content);
    let uses = use_statements(&lines);
    let mut shadowed: BTreeSet<String> = lines.iter().filter_map(|(_, c)| mod_name(c)).collect();
    for (_, paths) in &uses {
        for (path, binding) in paths {
            if path.len() > 1 || path[0] != *binding {
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

/// `mod x;` / `pub mod x {` → `x`.
fn mod_name(code: &str) -> Option<String> {
    let t = code.trim_start();
    let t = t
        .strip_prefix("pub")
        .map_or(t, |r| r.trim_start_matches(|c| c != ' '));
    let rest = t.trim_start().strip_prefix("mod ")?;
    let end = rest.find(|c: char| !is_ident(c)).unwrap_or(rest.len());
    (end > 0).then(|| rest[..end].to_string())
}

/// Each `use … ;` statement: the lines it spans and its flattened paths, each with
/// the name it binds.
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

/// The tree after `use` on a line that opens a use statement, else `None`.
fn use_tree_start(code: &str) -> Option<&str> {
    let t = code.trim_start();
    let t = match t.strip_prefix("pub") {
        Some(r) if r.starts_with('(') => r.split_once(')')?.1.trim_start(),
        Some(r) => r.trim_start(),
        None => t,
    };
    t.strip_prefix("use ")
}

/// Flatten a use tree (`a::{b, c::{d as e}}`) into `(path, binding)` pairs. A glob
/// binds nothing and is dropped; `self` binds the segment before it. Shared with
/// the `api-parity` rule's alias scan.
pub fn flatten(tree: &str) -> Paths {
    let tokens = tokenize(tree);
    let mut out = Vec::new();
    let mut i = 0;
    walk(&tokens, &mut i, &[], &mut out);
    out
}

fn walk(tokens: &[String], i: &mut usize, prefix: &[String], out: &mut Paths) {
    let mut path = prefix.to_vec();
    while let Some(tok) = tokens.get(*i) {
        *i += 1;
        match tok.as_str() {
            "::" => {}
            "{" => {
                while *i < tokens.len() && tokens[*i] != "}" {
                    walk(tokens, i, &path, out);
                    if tokens.get(*i).is_some_and(|t| t == ",") {
                        *i += 1;
                    }
                }
                *i += 1;
                return;
            }
            "*" => return,
            "as" => {
                if let (Some(alias), false) = (tokens.get(*i), path.is_empty()) {
                    *i += 1;
                    out.push((path, alias.clone()));
                }
                return;
            }
            "," | "}" => {
                *i -= 1;
                break;
            }
            seg if seg == "self" && !path.is_empty() => {}
            seg => path.push(seg.to_string()),
        }
    }
    if let Some(last) = path.last().cloned() {
        out.push((path, last));
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

/// The leading segment of every `name::` path on a line: an identifier right before
/// a `::` that is not itself the tail of a longer path (`a::name::`).
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
