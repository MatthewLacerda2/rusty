//! What one source file reaches, resolved the way rustc resolves it: the files its
//! `mod name;` lines load and the files its `include!` / `include_str!` name.
//!
//! A module's children sit under its *directory path*: a `mod.rs`, a crate root or
//! a `#[path]`-loaded file owns its own directory; any other `a.rs` puts its
//! children under `a/`. An inline `mod b { … }` adds `b/`. A `#[path = "p"]` on
//! `mod x;` is `p` from that directory path, the `a/` offset of a non-`mod.rs`
//! file aside (the Reference's *The path attribute*). Comments and literals are
//! read through `crate::code`, so a `mod x;` in a string or a doc is not one.

use crate::code::{bare_lines, code_lines, is_ident};
use std::path::{Component, Path, PathBuf};

/// A file one source file reaches.
#[derive(Debug, PartialEq)]
pub enum Reach {
    /// Loaded by `mod`: parsed in turn, owning its directory when `mod_rs`.
    Module { path: String, mod_rs: bool },
    /// Named by `include!` / `include_str!`: compiled or read, never parsed here.
    Included(String),
}

#[derive(Debug, PartialEq)]
enum Tok {
    Ident(String),
    Punct(char),
    Str(String),
}

/// Where a module's children go: its directory, plus the `a/` of a non-`mod.rs`
/// file `a.rs`, which a `#[path]` on its `mod` skips.
#[derive(Clone)]
struct Dir {
    path: PathBuf,
    relative: Option<String>,
}

impl Dir {
    fn children(&self) -> PathBuf {
        match &self.relative {
            Some(a) => self.path.join(a),
            None => self.path.clone(),
        }
    }
}

/// What the file at `path` (repo-relative) reaches, given its `text`; `exists`
/// says whether a candidate file is there.
pub fn reached(path: &str, mod_rs: bool, text: &str, exists: impl Fn(&str) -> bool) -> Vec<Reach> {
    let here = Path::new(path)
        .parent()
        .unwrap_or(Path::new(""))
        .to_path_buf();
    let stem = Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned());
    let mut dir = Dir {
        path: here.clone(),
        relative: if mod_rs { None } else { stem },
    };
    let toks = tokens(text);
    let (mut out, mut frames, mut depth) = (Vec::new(), Vec::new(), 0usize);
    let mut path_attr: Option<String> = None;
    let mut i = 0;
    while i < toks.len() {
        let at = |k: usize| toks.get(i + k);
        match &toks[i] {
            Tok::Punct('#') => {
                let (end, found) = attribute(&toks, i + 1);
                path_attr = found.or(path_attr);
                i = end;
                continue;
            }
            Tok::Ident(m) if m == "mod" => match (at(1), at(2)) {
                (Some(Tok::Ident(name)), Some(Tok::Punct(';'))) => {
                    out.extend(external(&dir, name, path_attr.take(), &exists));
                    i += 3;
                    continue;
                }
                (Some(Tok::Ident(name)), Some(Tok::Punct('{'))) => {
                    frames.push((depth, dir.clone()));
                    let path = match path_attr.take() {
                        Some(p) => dir.path.join(p),
                        None => dir.children().join(name),
                    };
                    dir = Dir {
                        path,
                        relative: None,
                    };
                    depth += 1;
                    i += 3;
                    continue;
                }
                _ => {}
            },
            Tok::Ident(m) if m == "include" || m == "include_str" => {
                if let (Some(Tok::Punct('!')), Some(Tok::Punct('(')), Some(Tok::Str(s))) =
                    (at(1), at(2), at(3))
                {
                    out.push(Reach::Included(normal(&here.join(s))));
                }
            }
            Tok::Punct('{') => {
                depth += 1;
                path_attr = None;
            }
            Tok::Punct('}') => {
                depth = depth.saturating_sub(1);
                if frames.last().is_some_and(|(d, _)| *d == depth) {
                    dir = frames.pop().map_or(dir, |(_, d)| d);
                }
                path_attr = None;
            }
            Tok::Punct(';') => path_attr = None,
            _ => {}
        }
        i += 1;
    }
    out
}

/// The files `mod name;` loads: its `#[path]`, or `name.rs` / `name/mod.rs`.
fn external(
    dir: &Dir,
    name: &str,
    path_attr: Option<String>,
    exists: impl Fn(&str) -> bool,
) -> Vec<Reach> {
    if let Some(p) = path_attr {
        let path = normal(&dir.path.join(p));
        return vec![Reach::Module { path, mod_rs: true }];
    }
    let base = dir.children();
    [
        (base.join(format!("{name}.rs")), false),
        (base.join(name).join("mod.rs"), true),
    ]
    .into_iter()
    .map(|(p, mod_rs)| (normal(&p), mod_rs))
    .filter(|(p, _)| exists(p))
    .map(|(path, mod_rs)| Reach::Module { path, mod_rs })
    .collect()
}

/// Past the attribute whose `#` is just before `from` (`#[…]` or `#![…]`), and
/// the `path = "…"` inside it, a `cfg_attr`'s included.
fn attribute(toks: &[Tok], from: usize) -> (usize, Option<String>) {
    let mut i = from;
    if toks.get(i) == Some(&Tok::Punct('!')) {
        i += 1;
    }
    if toks.get(i) != Some(&Tok::Punct('[')) {
        return (from, None);
    }
    let (mut depth, mut found) = (0usize, None);
    while let Some(t) = toks.get(i) {
        match t {
            Tok::Punct('[') => depth += 1,
            Tok::Punct(']') => {
                depth -= 1;
                if depth == 0 {
                    return (i + 1, found);
                }
            }
            Tok::Ident(p) if p == "path" => {
                if let (Some(Tok::Punct('=')), Some(Tok::Str(s))) =
                    (toks.get(i + 1), toks.get(i + 2))
                {
                    found = Some(s.clone());
                }
            }
            _ => {}
        }
        i += 1;
    }
    (i, found)
}

/// `text` as identifiers, punctuation and one-line string literals, comments
/// dropped: structure from [`bare_lines`], a literal's text from [`code_lines`].
fn tokens(text: &str) -> Vec<Tok> {
    let mut toks = Vec::new();
    for (code, bare) in code_lines(text).iter().zip(bare_lines(text)) {
        let code: Vec<char> = code.chars().collect();
        let bare: Vec<char> = bare.chars().collect();
        let mut k = 0;
        while k < bare.len() {
            let c = bare[k];
            if c.is_whitespace() {
                if code.get(k) == Some(&'"') {
                    let (s, end) = literal(&code, k + 1);
                    toks.push(Tok::Str(s));
                    k = end;
                } else {
                    k += 1;
                }
            } else if is_ident(c) {
                let start = k;
                while k < bare.len() && is_ident(bare[k]) {
                    k += 1;
                }
                toks.push(Tok::Ident(bare[start..k].iter().collect()));
            } else {
                toks.push(Tok::Punct(c));
                k += 1;
            }
        }
    }
    toks
}

/// A string literal's text from `from` (just past its `"`), and where it ends.
fn literal(code: &[char], from: usize) -> (String, usize) {
    let mut out = String::new();
    let mut k = from;
    while let Some(&c) = code.get(k) {
        k += 1;
        match c {
            '"' => break,
            '\\' => {
                if let Some(&n) = code.get(k) {
                    out.push(n);
                    k += 1;
                }
            }
            c => out.push(c),
        }
    }
    (out, k)
}

/// `p` with its `.` and `..` folded, `/`-separated.
pub fn normal(p: &Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    for c in p.components() {
        match c {
            Component::ParentDir if parts.last().is_some_and(|l| l != "..") => {
                parts.pop();
            }
            Component::Normal(s) => parts.push(s.to_string_lossy().into_owned()),
            Component::ParentDir => parts.push("..".into()),
            _ => {}
        }
    }
    parts.join("/")
}
