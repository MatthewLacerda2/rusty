//! Where an item ends, for the scans that skip test-gated items (#1113): the layering
//! lint's `live_lines` and the rules' test-code classifier (`rules/test_code.rs`).
//! A line scan, not a parser: brackets are counted outside comments and string and
//! char literals and `/* */` comments (see `crate::code`).

use crate::code::{code_of, cut, scan, strip_comment, Open};
use std::borrow::Cow;

/// The index of the last line of the item starting at `lines[from]` (its
/// attributes included). A `fn`, `impl`, `mod`, `struct`, `use`, … ends where its
/// brackets balance on a line ending in `;` or `}`, so a wrapped signature or a
/// `where` clause never ends it early. Anything else (a field, a variant, a match
/// arm, a statement) may also end on `,`; one whose parent closes first (a last
/// field without a trailing comma) ends on the line before that close. The end of
/// `lines` closes an item that never ends.
pub fn item_end(lines: &[&str], from: usize) -> usize {
    let item = head(lines, from).is_some_and(|h| is_item_head(&h));
    let ends: &[char] = if item { &[';', '}'] } else { &[';', '}', ','] };
    let (mut depth, mut open) = (0i32, Open::None);
    for (i, raw) in lines.iter().enumerate().skip(from) {
        let (code, delta) = brackets(raw, &mut open);
        depth += delta;
        if depth < 0 {
            return i.saturating_sub(1).max(from);
        }
        if depth == 0
            && matches!(open, Open::None | Open::Block(_))
            && code.trim_end().ends_with(ends)
        {
            return i;
        }
    }
    lines.len().saturating_sub(1)
}

/// The first code line of the item at `lines[from]` past its attributes.
fn head(lines: &[&str], from: usize) -> Option<String> {
    let mut at = from;
    while at < lines.len() {
        let item = attributes(&lines[at..]);
        let code = strip_comment(item.code, &mut Open::None);
        if !code.trim().is_empty() {
            return Some(code.trim().to_string());
        }
        at += item.line + 1;
    }
    None
}

/// What [`attributes`] reads: the `#[…]` attributes leading an item, and where its
/// own code starts.
pub struct Attributes<'a> {
    /// Each attribute from `#[` to its closing `]`, wrapped lines joined by a space.
    pub attrs: Vec<String>,
    /// The line (an index into the scanned lines) the item's code starts on.
    pub line: usize,
    /// That line's code past the attributes; a line with no attribute is all code.
    pub code: &'a str,
}

/// The attributes opening `lines[0]` and the code past them: the one attribute
/// reader the scans share (#1136). Brackets count outside comments and literals
/// (`scan`), so `#[doc = "a (b"]` closes where it should, and an attribute may
/// share its line with the next one or with its item (`#[cfg(test)] use a::B;`).
/// An attribute ends on bracket depth alone, however many lines it wraps (a long
/// `#[derive(…)]`); one that never closes is read as code.
pub fn attributes<'a>(lines: &[&'a str]) -> Attributes<'a> {
    let mut item = Attributes {
        attrs: Vec::new(),
        line: 0,
        code: lines.first().map_or("", |l| l.trim_start()),
    };
    while let Some((attr, more, rest)) =
        attribute(item.code, &lines[(item.line + 1).min(lines.len())..])
    {
        item.attrs.push(attr);
        item.line += more;
        item.code = rest;
        if rest.is_empty() && item.line + 1 < lines.len() {
            item.line += 1;
            item.code = lines[item.line].trim_start();
        }
    }
    item
}

/// The attribute opening `first` (then running on through `next`): its text, how
/// many of `next` it takes, and its last line's code after it.
fn attribute<'a>(first: &'a str, next: &[&'a str]) -> Option<(String, usize, &'a str)> {
    if !first.starts_with("#[") {
        return None;
    }
    let (mut depth, mut open, mut text) = (0i32, Open::None, String::new());
    for (n, line) in std::iter::once(&first).chain(next).enumerate() {
        let mut close = None;
        let (code_end, cuts) = scan(line, &mut open, |at, c| match c {
            _ if close.is_some() => {}
            '{' | '(' | '[' => depth += 1,
            '}' | ')' | ']' => {
                depth -= 1;
                if c == ']' && depth == 0 {
                    close = Some(at + 1);
                }
            }
            _ => {}
        });
        if let Some(at) = close {
            text.push_str(cut(line, at, &cuts).trim());
            return Some((text, n, line[at..code_end.max(at)].trim()));
        }
        text.push_str(cut(line, code_end, &cuts).trim());
        text.push(' ');
    }
    None
}

/// Does `code` open an item (`pub(crate) unsafe fn`, `impl<T>`, `mod x`, …)
/// rather than a field, variant, arm or statement? `union` and `macro_rules` are
/// item heads only before an item (`union X {`, `macro_rules! x`): a field may be
/// named `union`.
fn is_item_head(code: &str) -> bool {
    let mut rest = code;
    loop {
        let word = rest.split(|c: char| !super::scan::is_ident(c)).next();
        let word = word.unwrap_or("");
        rest = rest[word.len()..].trim_start();
        match word {
            "pub" if rest.starts_with('(') => {
                rest = rest.find(')').map_or("", |at| rest[at + 1..].trim_start());
            }
            "pub" | "unsafe" | "async" | "default" => {}
            "fn" | "impl" | "mod" | "struct" | "enum" | "trait" | "use" | "const" | "static"
            | "type" | "extern" => return true,
            "union" => return rest.starts_with(super::scan::is_ident),
            "macro_rules" => return rest.starts_with('!'),
            _ => return false,
        }
    }
}

/// `line`'s code (its comments cut) and its opened minus closed brackets of
/// every kind, outside literals, starting inside `open` and leaving it as the line
/// ends.
fn brackets<'a>(line: &'a str, open: &mut Open) -> (Cow<'a, str>, i32) {
    let mut delta = 0;
    let code = code_of(line, open, |_, c| match c {
        '{' | '(' | '[' => delta += 1,
        '}' | ')' | ']' => delta -= 1,
        _ => {}
    });
    (code, delta)
}

#[cfg(test)]
#[path = "extent_tests.rs"]
mod tests;
