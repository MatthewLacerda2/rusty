//! What one source file reaches, resolved the way rustc resolves it: the files its
//! `mod name;` lines load and the files its `include!` / `include_str!` name.
//!
//! A module's children sit under its *directory path*: a `mod.rs`, a crate root or
//! a `#[path]`-loaded file owns its own directory; any other `a.rs` puts its
//! children under `a/`. An inline `mod b { … }` adds `b/`. A `#[path = "p"]` on
//! `mod x;` is `p` from that directory path, the `a/` offset of a non-`mod.rs`
//! file aside (the Reference's *The path attribute*). Comments and literals are
//! read through `crate::code`, so a `mod x;` in a string or a doc is not one.

use crate::code::{is_ident, walk, Open, Piece};
use std::path::{Path, PathBuf};

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
    let mut path_attr: Option<PathAttr> = None;
    let mut i = 0;
    while i < toks.len() {
        let at = |k: usize| toks.get(i + k);
        match &toks[i] {
            Tok::Punct('#') => {
                let attr = attribute(&toks, i + 1);
                path_attr = attr.path.or(path_attr);
                let named = attr.includes.iter().map(|s| normal(&here.join(s)));
                out.extend(named.map(Reach::Included));
                i = attr.end.max(i + 1);
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
                        Some(p) => dir.path.join(p.path),
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
            Tok::Ident(_) => {
                out.extend(included(&toks, i).map(|s| Reach::Included(normal(&here.join(s)))))
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

/// The files `mod name;` loads: its `#[path]`, or `name.rs` / `name/mod.rs` (both,
/// for a `cfg_attr` path).
fn external(
    dir: &Dir,
    name: &str,
    path_attr: Option<PathAttr>,
    exists: impl Fn(&str) -> bool,
) -> Vec<Reach> {
    let mut out = Vec::new();
    if let Some(p) = path_attr {
        let path = normal(&dir.path.join(p.path));
        out.push(Reach::Module { path, mod_rs: true });
        if !p.conditional {
            return out;
        }
    }
    let base = dir.children();
    out.extend(
        [
            (base.join(format!("{name}.rs")), false),
            (base.join(name).join("mod.rs"), true),
        ]
        .into_iter()
        .map(|(p, mod_rs)| (normal(&p), mod_rs))
        .filter(|(p, _)| exists(p))
        .map(|(path, mod_rs)| Reach::Module { path, mod_rs }),
    );
    out
}

/// A `mod`'s `path = "…"`: `conditional` when a `cfg_attr` sets it, so the builds
/// it is off in load the default file.
struct PathAttr {
    path: String,
    conditional: bool,
}

/// What one attribute holds: where it ends, the `p` of a `#[path = "p"]` (or a
/// `cfg_attr`'s `path = "p"`), and the files `include!` / `include_str!` name in it.
struct Attribute {
    end: usize,
    path: Option<PathAttr>,
    includes: Vec<String>,
}

/// The attribute whose `#` is just before `from` (`#[…]` or `#![…]`).
fn attribute(toks: &[Tok], from: usize) -> Attribute {
    let mut i = from;
    if toks.get(i) == Some(&Tok::Punct('!')) {
        i += 1;
    }
    let mut out = Attribute {
        end: from,
        path: None,
        includes: Vec::new(),
    };
    if toks.get(i) != Some(&Tok::Punct('[')) {
        return out;
    }
    let name = match toks.get(i + 1) {
        Some(Tok::Ident(n)) => n.as_str(),
        _ => "",
    };
    let mut depth = 0usize;
    while let Some(t) = toks.get(i) {
        let before = &toks[i - 1];
        match t {
            Tok::Punct('[' | '(') => depth += 1,
            Tok::Punct(']' | ')') => {
                depth -= 1;
                if depth == 0 {
                    out.end = i + 1;
                    return out;
                }
            }
            Tok::Ident(p) if p == "path" => {
                let named = match name {
                    "path" => depth == 1,
                    "cfg_attr" => depth == 2 && *before == Tok::Punct(','),
                    _ => false,
                };
                if let (true, Some(Tok::Punct('=')), Some(Tok::Str(s))) =
                    (named, toks.get(i + 1), toks.get(i + 2))
                {
                    out.path = Some(PathAttr {
                        path: s.clone(),
                        conditional: name == "cfg_attr",
                    });
                }
            }
            _ => out.includes.extend(included(toks, i)),
        }
        i += 1;
    }
    out.end = i;
    out
}

/// The file an `include!("…")` or `include_str!("…")` starting at `i` names.
fn included(toks: &[Tok], i: usize) -> Option<String> {
    match (&toks[i], toks.get(i + 1), toks.get(i + 2), toks.get(i + 3)) {
        (Tok::Ident(m), Some(Tok::Punct('!')), Some(Tok::Punct('(')), Some(Tok::Str(s)))
            if m == "include" || m == "include_str" =>
        {
            Some(s.clone())
        }
        _ => None,
    }
}

/// `text` as identifiers, punctuation and string literals (a multi-line one
/// joined by `\n`), comments and char literals dropped: one [`walk`] of each
/// line, so a literal is exactly what the scanner reads as one.
fn tokens(text: &str) -> Vec<Tok> {
    let (mut toks, mut open, mut string) = (Vec::new(), Open::None, None::<String>);
    for line in text.lines() {
        // Just past the last code char: an identifier goes on only from there.
        let mut glued = usize::MAX;
        walk(line, &mut open, |piece| match piece {
            Piece::Code(at, c) => {
                let joined = at == glued;
                glued = if c.is_whitespace() {
                    usize::MAX
                } else {
                    at + c.len_utf8()
                };
                match (toks.last_mut(), is_ident(c)) {
                    (Some(Tok::Ident(s)), true) if joined => s.push(c),
                    (_, true) => toks.push(Tok::Ident(c.into())),
                    _ if c.is_whitespace() => {}
                    _ => toks.push(Tok::Punct(c)),
                }
            }
            Piece::Str { body, raw, closed } => {
                glued = usize::MAX;
                let continued = string.is_some();
                let s = string.get_or_insert_with(String::new);
                if continued {
                    s.push('\n');
                }
                unescape(&line[body], raw, s);
                if closed {
                    toks.extend(string.take().map(Tok::Str));
                }
            }
        });
    }
    toks
}

/// A literal body's text onto `out`; a backslash keeps the char after it, unless
/// the literal is `raw`.
fn unescape(body: &str, raw: bool, out: &mut String) {
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' if !raw => out.extend(chars.next()),
            c => out.push(c),
        }
    }
}

/// `p` with its `.` and `..` folded ([`crate::layers::normalise`]), `/`-separated.
pub fn normal(p: &Path) -> String {
    crate::layers::normalise(p).to_string_lossy().into_owned()
}
