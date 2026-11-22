use std::borrow::Cow;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Open {
    None,
    Escaped,
    Raw(usize),
    Block(usize),
}

pub fn strip_comment<'a>(line: &'a str, open: &mut Open) -> Cow<'a, str> {
    code_of(line, open, |_, _| {})
}

pub fn code_lines(source: &str) -> Vec<Cow<'_, str>> {
    let mut open = Open::None;
    source
        .lines()
        .map(|l| strip_comment(l, &mut open))
        .collect()
}

pub fn bare_lines(source: &str) -> Vec<String> {
    let mut open = Open::None;
    source
        .lines()
        .map(|line| {
            let mut bare: Vec<char> = Vec::new();
            let code = code_of(line, &mut open, |at, c| {
                let col = line[..at].chars().count();
                bare.resize(col, ' ');
                bare.push(c);
            });
            bare.resize(code.chars().count(), ' ');
            bare.into_iter().collect()
        })
        .collect()
}

pub fn code_of<'a>(line: &'a str, open: &mut Open, f: impl FnMut(usize, char)) -> Cow<'a, str> {
    let (end, cuts) = scan(line, open, f);
    cut(line, end, &cuts)
}

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

pub fn scan(
    line: &str,
    open: &mut Open,
    mut f: impl FnMut(usize, char),
) -> (usize, Vec<(usize, usize)>) {
    walk(line, open, |piece| {
        if let Piece::Code(at, c) = piece {
            f(at, c);
        }
    })
}

#[derive(Clone, Debug, PartialEq)]
pub enum Piece {
    Code(usize, char),
    Str {
        body: std::ops::Range<usize>,
        raw: bool,
        closed: bool,
    },
}

pub fn walk(line: &str, open: &mut Open, mut f: impl FnMut(Piece)) -> (usize, Vec<(usize, usize)>) {
    let bytes = line.as_bytes();
    let body = |from: usize, to: Option<usize>, kind: Open| Piece::Str {
        body: from..to.map_or(bytes.len(), |next| next - quote_len(kind)),
        raw: matches!(kind, Open::Raw(_)),
        closed: to.is_some(),
    };
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
        kind => {
            let next = close(line, 0, kind);
            f(body(0, next, kind));
            match next {
                Some(next) => (i, *open) = (next, Open::None),
                None => return (bytes.len(), cuts),
            }
        }
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
        if let Some((from, kind)) = literal {
            let next = close(line, from, kind);
            f(body(from, next, kind));
            match next {
                Some(next) => i = next,
                None => {
                    *open = kind;
                    return (bytes.len(), cuts);
                }
            }
            continue;
        }
        let c = rest.chars().next().unwrap_or(' ');
        f(Piece::Code(i, c));
        i += c.len_utf8();
    }
    (bytes.len(), cuts)
}

fn quote_len(kind: Open) -> usize {
    match kind {
        Open::Raw(hashes) => hashes + 1,
        _ => 1,
    }
}

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

fn raw_prefix(line: &str, at: usize) -> bool {
    at >= 1 && matches!(line.as_bytes()[at - 1], b'b' | b'c') && !ident_before(line, at - 1)
}

fn raw_start(line: &str, from: usize) -> Option<(usize, Open)> {
    let hashes = line[from..].bytes().take_while(|&b| b == b'#').count();
    (line.as_bytes().get(from + hashes) == Some(&b'"'))
        .then_some((from + hashes + 1, Open::Raw(hashes)))
}

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

pub(crate) fn char_end(line: &str, from: usize) -> Option<usize> {
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
