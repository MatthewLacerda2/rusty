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
    // The tokens not read yet. Each step drops at least the one it read, so the
    // scan ends on any input: a step that read nothing would loop forever.
    let mut rest = toks.as_slice();
    while let Some(tok) = rest.first() {
        let at = |k: usize| rest.get(k);
        match tok {
            // An attribute is skipped whole, by its brackets, so a `;` or a brace
            // inside it (`#[foo([u8; 4])]`) never ends the item a `#[path]` sits
            // on; its `include!`s are still read.
            Tok::Punct('#') => {
                // `#![…]`, an inner attribute, never sets a `mod`'s path
                let inner = rest.get(1) == Some(&Tok::Punct('!'));
                let attr = &rest[1 + usize::from(inner)..];
                let (found, len) = attribute(attr);
                path_attr = found.filter(|_| !inner).or(path_attr);
                if len > 0 {
                    out.extend(
                        (0..len)
                            .filter_map(|k| included(&attr[k..len]))
                            .map(|s| Reach::Included(normal(&here.join(s)))),
                    );
                    rest = &attr[len..];
                    continue;
                }
            }
            Tok::Ident(m) if m == "mod" => match (at(1), at(2)) {
                (Some(Tok::Ident(name)), Some(Tok::Punct(';'))) => {
                    out.extend(external(&dir, name, path_attr.take(), &exists));
                    rest = &rest[3..];
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
                    rest = &rest[3..];
                    continue;
                }
                _ => {}
            },
            Tok::Ident(_) => {
                out.extend(included(rest).map(|s| Reach::Included(normal(&here.join(s)))))
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
        rest = &rest[1..];
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

/// The `p` of the `#[path = "p"]` (or a `cfg_attr`'s `path = "p"`) whose `[`
/// opens `toks`, if that is what it is, and how many tokens the attribute spans,
/// its closing `]` included (all of `toks` when it never closes; 0 when no `[`
/// opens it). Every bracket kind nests, so a `;` or brace inside stays inside.
fn attribute(toks: &[Tok]) -> (Option<PathAttr>, usize) {
    if toks.first() != Some(&Tok::Punct('[')) {
        return (None, 0);
    }
    let name = match toks.get(1) {
        Some(Tok::Ident(n)) => n.as_str(),
        _ => "",
    };
    let (mut depth, mut out) = (0usize, None);
    for (i, t) in toks.iter().enumerate() {
        match t {
            Tok::Punct('[' | '(' | '{') => depth += 1,
            Tok::Punct(']' | ')' | '}') => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return (out, i + 1);
                }
            }
            Tok::Ident(p) if p == "path" => {
                let named = match name {
                    "path" => depth == 1,
                    "cfg_attr" => depth == 2 && toks[i - 1] == Tok::Punct(','),
                    _ => false,
                };
                if let (true, Some(Tok::Punct('=')), Some(Tok::Str(s))) =
                    (named, toks.get(i + 1), toks.get(i + 2))
                {
                    out = Some(PathAttr {
                        path: s.clone(),
                        conditional: name == "cfg_attr",
                    });
                }
            }
            _ => {}
        }
    }
    (out, toks.len())
}

/// The file an `include!("…")` or `include_str!("…")` opening `toks` names.
fn included(toks: &[Tok]) -> Option<String> {
    match (toks.first()?, toks.get(1), toks.get(2), toks.get(3)) {
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

#[cfg(test)]
#[path = "module_tree_tests.rs"]
mod tests;
