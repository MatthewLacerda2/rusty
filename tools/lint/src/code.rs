//! The one comment stripper (#1199): every rule that scans source text reads a
//! line's code through [`strip_comment`], so none cuts at a `//` inside a string
//! (`"http://x"`) or reads past a `/* */`. A line scan, not a parser: string and
//! char literals and block comments (which nest, as Rust's do) are carried from one
//! line to the next in [`Open`].

use std::borrow::Cow;

/// A literal or block comment still open at the end of a line.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Open {
    None,
    /// `"…`, closed by an unescaped `"`.
    Escaped,
    /// `r#"…`, closed by `"` and this many `#`s.
    Raw(usize),
    /// `/*…`, this many comments deep.
    Block(usize),
}

/// `line` as code: its `//` comment cut and its `/* */` comments dropped (each a
/// single space), starting inside `open` and leaving it as the line ends. A `//` or
/// `/*` inside a string or char literal is kept.
pub fn strip_comment<'a>(line: &'a str, open: &mut Open) -> Cow<'a, str> {
    code_of(line, open, |_, _| {})
}

/// Every line of `source` as [`strip_comment`] reads it, a literal or block comment
/// carried open across lines.
pub fn code_lines(source: &str) -> Vec<Cow<'_, str>> {
    let mut open = Open::None;
    source
        .lines()
        .map(|l| strip_comment(l, &mut open))
        .collect()
}

/// [`strip_comment`], also calling `f` with each char (and its byte offset in
/// `line`) that is code outside string and char literals.
pub fn code_of<'a>(line: &'a str, open: &mut Open, f: impl FnMut(usize, char)) -> Cow<'a, str> {
    let (end, cuts) = scan(line, open, f);
    cut(line, end, &cuts)
}

/// `line[..end]` with the block comments in `cuts` (from [`scan`]) each replaced by
/// a space; a comment starting at or past `end` is not in it.
pub fn cut<'a>(line: &'a str, end: usize, cuts: &[(usize, usize)]) -> Cow<'a, str> {
    let mut cuts = cuts.iter().filter(|c| c.0 < end).peekable();
    if cuts.peek().is_none() {
        return Cow::Borrowed(&line[..end]);
    }
    let (mut out, mut at) = (String::new(), 0);
    for &(from, to) in cuts {
        out.push_str(&line[at..from]);
        out.push(' ');
        at = to.min(end);
    }
    out.push_str(&line[at..end]);
    Cow::Owned(out)
}

pub fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Walks `line` from inside `open`, calling `f` with each char (and its byte
/// offset) of code outside literals and comments. Returns where the code ends (the
/// first `//` outside a literal, or the line's end) and the byte ranges of the
/// block comments before it. A literal or comment still open at the end is left in
/// `open`.
pub fn scan(
    line: &str,
    open: &mut Open,
    mut f: impl FnMut(usize, char),
) -> (usize, Vec<(usize, usize)>) {
    let bytes = line.as_bytes();
    let mut cuts = Vec::new();
    let mut i = 0;
    match *open {
        Open::None => {}
        Open::Block(depth) => match block_end(line, 0, depth) {
            Ok(next) => {
                (i, *open) = (next, Open::None);
                cuts.push((0, next));
            }
            Err(depth) => {
                *open = Open::Block(depth);
                return (bytes.len(), vec![(0, bytes.len())]);
            }
        },
        kind => match close(line, 0, kind) {
            Some(next) => (i, *open) = (next, Open::None),
            None => return (bytes.len(), cuts),
        },
    }
    while i < bytes.len() {
        let rest = &line[i..];
        if rest.starts_with("//") {
            return (i, cuts);
        }
        if rest.starts_with("/*") {
            match block_end(line, i + 2, 1) {
                Ok(next) => cuts.push((i, next)),
                Err(depth) => {
                    *open = Open::Block(depth);
                    cuts.push((i, bytes.len()));
                    return (bytes.len(), cuts);
                }
            }
            i = cuts.last().map_or(i, |c| c.1);
            continue;
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
                    return (bytes.len(), cuts);
                }
            }
            continue;
        }
        let c = rest.chars().next().unwrap_or(' ');
        f(i, c);
        i += c.len_utf8();
    }
    (bytes.len(), cuts)
}

/// Past the `*/` that closes a block comment `depth` deep, whose body resumes at
/// `from`; or, when `line` ends first, how deep it is still.
fn block_end(line: &str, from: usize, mut depth: usize) -> Result<usize, usize> {
    let bytes = line.as_bytes();
    let mut i = from;
    while i + 1 < bytes.len() {
        match &bytes[i..i + 2] {
            b"/*" => (depth, i) = (depth + 1, i + 2),
            b"*/" => {
                depth -= 1;
                i += 2;
                if depth == 0 {
                    return Ok(i);
                }
            }
            _ => i += 1,
        }
    }
    Err(depth)
}

fn ident_before(line: &str, at: usize) -> bool {
    line[..at].chars().next_back().is_some_and(is_ident)
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
#[path = "code_tests.rs"]
mod tests;
