//! Where an item ends, for the scans that skip test-gated items (#1113): the layering
//! lint's `live_lines` and the rules' test-code classifier (`rules/test_code.rs`).
//! A line scan, not a parser: brackets are counted outside comments and string and
//! char literals, a string carried open from one line to the next. A `/* */`
//! comment is read as code.

/// The index of the last line of the item starting at `lines[from]` (its
/// attributes included). A `fn`, `impl`, `mod`, `struct`, `use`, … ends where its
/// brackets balance on a line ending in `;` or `}`, so a wrapped signature or a
/// `where` clause never ends it early. Anything else (a field, a variant, a match
/// arm, a statement) may also end on `,`; one whose parent closes first (a last
/// field without a trailing comma) ends on the line before that close. The end of
/// `lines` closes an item that never ends.
pub fn item_end(lines: &[&str], from: usize) -> usize {
    let item = head(lines, from).is_some_and(is_item_head);
    let ends: &[char] = if item { &[';', '}'] } else { &[';', '}', ','] };
    let (mut depth, mut open) = (0i32, Open::None);
    for (i, raw) in lines.iter().enumerate().skip(from) {
        let (code, delta) = brackets(raw, &mut open);
        depth += delta;
        if depth < 0 {
            return i.saturating_sub(1).max(from);
        }
        if depth == 0 && open == Open::None && code.trim_end().ends_with(ends) {
            return i;
        }
    }
    lines.len().saturating_sub(1)
}

/// The first code line of the item at `lines[from]` past its attributes.
fn head<'a>(lines: &[&'a str], from: usize) -> Option<&'a str> {
    let (mut depth, mut open) = (0i32, Open::None);
    for raw in &lines[from..] {
        let in_attr = depth > 0 || open != Open::None;
        let (code, delta) = brackets(raw, &mut open);
        depth += delta;
        if in_attr {
            continue;
        }
        // An attribute and its item can share a line: `#[cfg(test)] use a::B;`.
        let code = strip_attrs(code.trim());
        if code.is_empty() {
            continue;
        }
        return Some(code);
    }
    None
}

/// `code` past its leading complete `#[…]` attributes; empty when an attribute
/// runs on past the line (its brackets carry `head` into the next lines).
fn strip_attrs(code: &str) -> &str {
    let mut rest = code;
    while rest.starts_with("#[") {
        let (mut depth, mut open) = (0i32, Open::None);
        let mut end = None;
        scan(rest, &mut open, |at, c| {
            depth += match c {
                '[' => 1,
                ']' => -1,
                _ => 0,
            };
            if c == ']' && depth == 0 && end.is_none() {
                end = Some(at + 1);
            }
        });
        let Some(end) = end else {
            return "";
        };
        rest = rest[end..].trim_start();
    }
    rest
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

/// A string literal still open at the end of a line.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Open {
    None,
    /// `"…`, closed by an unescaped `"`.
    Escaped,
    /// `r#"…`, closed by `"` and this many `#`s.
    Raw(usize),
}

/// `line`'s code (its `//` comment cut) and its opened minus closed brackets of
/// every kind, outside literals, starting inside `open` and leaving it as the line
/// ends.
fn brackets<'a>(line: &'a str, open: &mut Open) -> (&'a str, i32) {
    let mut delta = 0;
    let end = scan(line, open, |_, c| match c {
        '{' | '(' | '[' => delta += 1,
        '}' | ')' | ']' => delta -= 1,
        _ => {}
    });
    (&line[..end], delta)
}

/// `line` without its `//` comment, starting inside `open` and leaving it as the
/// line ends; a `//` inside a string literal is kept.
pub fn strip_comment<'a>(line: &'a str, open: &mut Open) -> &'a str {
    &line[..scan(line, open, |_, _| {})]
}

/// Walks `line` from inside `open`, calling `f` with each char (and its byte
/// offset) outside string and char literals, and returns where its code ends: the first `//` outside a
/// literal, or the line's end. A string still open at the end is left in `open`.
fn scan(line: &str, open: &mut Open, mut f: impl FnMut(usize, char)) -> usize {
    let bytes = line.as_bytes();
    let mut i = 0;
    if *open != Open::None {
        match close(line, 0, *open) {
            Some(next) => (i, *open) = (next, Open::None),
            None => return bytes.len(),
        }
    }
    while i < bytes.len() {
        let rest = &line[i..];
        if rest.starts_with("//") {
            return i;
        }
        let literal = match bytes[i] {
            b'"' => Some((i + 1, Open::Escaped)),
            b'r' if !ident_before(line, i) || raw_prefix(line, i) => raw_start(line, i + 1),
            b'\'' => {
                if let Some(next) = char_end(line, i + 1) {
                    i = next;
                    continue;
                }
                None
            }
            _ => None,
        };
        if let Some((body, kind)) = literal {
            match close(line, body, kind) {
                Some(next) => i = next,
                None => {
                    *open = kind;
                    return bytes.len();
                }
            }
            continue;
        }
        let c = rest.chars().next().unwrap_or(' ');
        f(i, c);
        i += c.len_utf8();
    }
    bytes.len()
}

fn ident_before(line: &str, at: usize) -> bool {
    line[..at]
        .chars()
        .next_back()
        .is_some_and(super::scan::is_ident)
}

/// Is the `r` at `at` the second letter of a `br"…"` or `cr"…"` raw string?
fn raw_prefix(line: &str, at: usize) -> bool {
    at >= 1 && matches!(line.as_bytes()[at - 1], b'b' | b'c') && !ident_before(line, at - 1)
}

/// After an `r` at `from - 1`: where a raw string's body starts, and its kind.
fn raw_start(line: &str, from: usize) -> Option<(usize, Open)> {
    let hashes = line[from..].bytes().take_while(|&b| b == b'#').count();
    (line.as_bytes().get(from + hashes) == Some(&b'"'))
        .then_some((from + hashes + 1, Open::Raw(hashes)))
}

/// Past the end of a `kind` literal whose body resumes at `from`, or `None` when it
/// stays open past the end of `line`.
fn close(line: &str, from: usize, kind: Open) -> Option<usize> {
    let bytes = line.as_bytes();
    let Open::Raw(hashes) = kind else {
        let mut i = from;
        while i < bytes.len() {
            match bytes[i] {
                b'\\' => i += 2,
                b'"' => return Some(i + 1),
                _ => i += 1,
            }
        }
        return None;
    };
    let end = format!("\"{}", "#".repeat(hashes));
    line[from..].find(&end).map(|at| from + at + end.len())
}

/// After a `'` at `from - 1`: past a char literal (`'x'`, `'\''`, `'\u{7f}'`), or
/// `None` for a lifetime (`'a`).
fn char_end(line: &str, from: usize) -> Option<usize> {
    let rest = &line[from..];
    if let Some(escaped) = rest.strip_prefix('\\') {
        let skip = escaped.chars().next()?.len_utf8();
        return escaped[skip..]
            .find('\'')
            .map(|at| from + 1 + skip + at + 1);
    }
    let c = rest.chars().next()?;
    rest[c.len_utf8()..]
        .starts_with('\'')
        .then(|| from + c.len_utf8() + 1)
}

#[cfg(test)]
#[path = "extent_tests.rs"]
mod tests;
