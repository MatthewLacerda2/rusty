//! Where literals start and end in code with its comments cut.

use super::Kind;
use crate::code::is_ident;
use std::ops::Range;

/// The length of the literal `rest` opens with; an unclosed one runs to its end.
pub(super) fn literal(kind: Kind, rest: &str, prev: Option<char>) -> Option<usize> {
    match kind {
        Kind::Rust => rust(rest, prev),
        Kind::Lua => quoted(rest).or_else(|| {
            let (level, len) = long_open(rest)?;
            Some(len + long_close(&rest[len..], level).unwrap_or(rest.len() - len))
        }),
        Kind::Wgsl => None,
    }
}

fn rust(rest: &str, prev: Option<char>) -> Option<usize> {
    if prev.is_some_and(is_ident) {
        return None;
    }
    let (prefix, body) = match rest.as_bytes() {
        [b'b' | b'c', b'r', ..] => (2, &rest[2..]),
        [b'r', ..] => (1, &rest[1..]),
        [b'b' | b'c', b'"' | b'\'', ..] => return Some(1 + rust(&rest[1..], None)?),
        _ => return quoted(rest).or_else(|| char_literal(rest)),
    };
    let hashes = body.bytes().take_while(|&b| b == b'#').count();
    let inner = body[hashes..].strip_prefix('"')?;
    let close = format!("\"{}", "#".repeat(hashes));
    let len = inner
        .find(&close)
        .map_or(inner.len(), |at| at + close.len());
    Some(prefix + hashes + 1 + len)
}

pub(super) fn quoted(rest: &str) -> Option<usize> {
    let quote = rest.chars().next().filter(|&c| c == '"' || c == '\'')?;
    let mut chars = rest.char_indices().skip(1);
    while let Some((at, c)) = chars.next() {
        if c == '\\' {
            chars.next();
        } else if c == quote {
            return Some(at + 1);
        }
    }
    Some(rest.len())
}

/// `None` for a lifetime.
fn char_literal(rest: &str) -> Option<usize> {
    let body = rest.strip_prefix('\'')?;
    if let Some(escaped) = body.strip_prefix('\\') {
        let skip = escaped.chars().next()?.len_utf8();
        return escaped[skip..].find('\'').map(|at| 2 + skip + at + 1);
    }
    let c = body.chars().next()?;
    body[c.len_utf8()..]
        .starts_with('\'')
        .then(|| 1 + c.len_utf8() + 1)
}

/// Lua's `[==[`: its level and length.
pub(super) fn long_open(rest: &str) -> Option<(usize, usize)> {
    let after = rest.strip_prefix('[')?;
    let level = after.bytes().take_while(|&b| b == b'=').count();
    after[level..]
        .starts_with('[')
        .then_some((level, level + 2))
}

pub(super) fn long_close(rest: &str, level: usize) -> Option<usize> {
    let close = format!("]{}]", "=".repeat(level));
    rest.find(&close).map(|at| at + close.len())
}

/// Whitespace outside literals dropped, or one space between two word chars.
pub(super) fn normalize(kind: Kind, code: &str) -> String {
    let mut out = String::new();
    let (mut rest, mut prev, mut gap) = (code, None::<char>, false);
    while let Some(c) = rest.chars().next() {
        if c.is_whitespace() {
            gap = true;
            rest = &rest[c.len_utf8()..];
            continue;
        }
        let len = literal(kind, rest, prev).unwrap_or(c.len_utf8());
        let piece = &rest[..len];
        if gap && prev.is_some_and(is_ident) && piece.starts_with(is_ident) {
            out.push(' ');
        }
        out.push_str(piece);
        (prev, gap, rest) = (piece.chars().next_back(), false, &rest[len..]);
    }
    out
}

/// Each `#[doc = …]` attribute's range, and its strings' ranges.
pub(super) fn doc_attributes(code: &str) -> Vec<(Range<usize>, Vec<Range<usize>>)> {
    let at = |rest: &str| code.len() - rest.len();
    let mut found = Vec::new();
    let (mut rest, mut prev) = (code, None::<char>);
    while let Some(c) = rest.chars().next() {
        if let Some(len) = rust(rest, prev) {
            (prev, rest) = (Some('"'), &rest[len..]);
            continue;
        }
        let Some(value) = rest.strip_prefix('#').and_then(doc_value) else {
            (prev, rest) = (Some(c), &rest[c.len_utf8()..]);
            continue;
        };
        let start = at(rest);
        rest = &rest[1 + value..];
        let mut strings = Vec::new();
        let mut depth = 1usize;
        while let Some(c) = rest.chars().next() {
            if let Some(len) = rust(rest, prev) {
                strings.push(at(rest)..at(rest) + len);
                (prev, rest) = (Some('"'), &rest[len..]);
                continue;
            }
            depth = match c {
                '[' => depth + 1,
                ']' => depth - 1,
                _ => depth,
            };
            (prev, rest) = (Some(c), &rest[c.len_utf8()..]);
            if depth == 0 {
                break;
            }
        }
        found.push((start..at(rest), strings));
    }
    found
}

fn doc_value(rest: &str) -> Option<usize> {
    let after = rest.strip_prefix('!').unwrap_or(rest).strip_prefix('[')?;
    let name = after.trim_start().strip_prefix("doc")?;
    let value = name.trim_start().strip_prefix('=')?;
    Some(rest.len() - value.len())
}
