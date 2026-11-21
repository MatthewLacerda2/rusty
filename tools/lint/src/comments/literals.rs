//! Where literals start and end in code with its comments cut.

use super::Kind;
use crate::code::{char_end, is_ident};
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
        // A `'` that opens no char literal is a lifetime: code, not a literal.
        [b'\'', ..] => return char_end(rest, 1),
        _ => return quoted(rest),
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

/// Characters that run together into one token (`&&`, `->`, `<<=`, `//`).
const JOINING: &str = "!#$%&*+-./:<=>?@^|~";

/// Whitespace between `prev` and `next` keeps one space when dropping it could
/// make a different token: two word characters, two operator characters, or a
/// word before a quote or `#` (`r "x"` is not `r"x"`).
fn keeps_gap(prev: char, next: char) -> bool {
    (is_ident(prev) && (is_ident(next) || "\"'#".contains(next)))
        || (JOINING.contains(prev) && JOINING.contains(next))
}

/// `code` with whitespace outside literals dropped, save where [`keeps_gap`];
/// and for each byte of the result, the line (from 0) its token starts on.
pub(super) fn normalize(kind: Kind, code: &str) -> (String, Vec<usize>) {
    let (mut out, mut lines) = (String::new(), Vec::new());
    let (mut rest, mut prev, mut gap, mut line) = (code, None::<char>, false, 0);
    while let Some(c) = rest.chars().next() {
        if c.is_whitespace() {
            gap = true;
            line += usize::from(c == '\n');
            rest = &rest[c.len_utf8()..];
            continue;
        }
        let len = literal(kind, rest, prev).unwrap_or(c.len_utf8());
        let piece = &rest[..len];
        if gap && prev.is_some_and(|p| keeps_gap(p, c)) {
            out.push(' ');
            lines.push(line);
        }
        out.push_str(piece);
        lines.extend(std::iter::repeat_n(line, piece.len()));
        line += piece.matches('\n').count();
        (prev, gap, rest) = (piece.chars().next_back(), false, &rest[len..]);
    }
    (out, lines)
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
