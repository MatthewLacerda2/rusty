use super::{SyntaxError, MAX_LEN};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Tok<'a> {
    Num(f32),
    Name(&'a str),
    Sym(&'static str),
    End,
}

/// The symbols, longest first so `<=` is not read as `<` then `=`.
const SYMS: &[&str] = &[
    "<=", ">=", "==", "!=", "<", ">", "+", "-", "*", "/", "^", "(", ")", ",", ".",
];

pub(super) fn tokens(src: &str) -> Result<Vec<(Tok<'_>, usize)>, SyntaxError> {
    let chars = src.chars().count();
    if chars > MAX_LEN {
        return Err(SyntaxError {
            what: format!("the formula is {chars} characters long"),
            column: None,
            fix: format!("the most is {MAX_LEN}; split it across nodes"),
        });
    }
    let mut out = Vec::new();
    // Byte offset → column: every token character is ASCII, so the column only
    // drifts from the offset past a non-ASCII character, which is refused.
    let mut rest = src;
    let mut column = 1;
    while let Some(c) = rest.chars().next() {
        let len = if c.is_whitespace() {
            c.len_utf8()
        } else if c.is_ascii_digit() || (c == '.' && next_is_digit(rest)) {
            let len = number_len(rest);
            out.push((Tok::Num(number(&rest[..len], column)?), column));
            len
        } else if c.is_ascii_alphabetic() || c == '_' {
            let len = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            out.push((Tok::Name(&rest[..len]), column));
            len
        } else if let Some(sym) = SYMS.iter().find(|s| rest.starts_with(**s)) {
            out.push((Tok::Sym(sym), column));
            sym.len()
        } else {
            return Err(SyntaxError {
                what: format!("unexpected character {c:?}"),
                column: Some(column),
                fix: "a formula holds numbers, names, + - * / ^, comparisons, \
                      parentheses and commas"
                    .into(),
            });
        };
        column += rest[..len].chars().count();
        rest = &rest[len..];
    }
    out.push((Tok::End, column));
    Ok(out)
}

fn next_is_digit(rest: &str) -> bool {
    rest[1..].starts_with(|c: char| c.is_ascii_digit())
}

fn number_len(rest: &str) -> usize {
    let b = rest.as_bytes();
    let digits = |mut i: usize| {
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        i
    };
    let mut i = digits(0);
    if b.get(i) == Some(&b'.') {
        i = digits(i + 1);
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        let sign = usize::from(matches!(b.get(i + 1), Some(b'+' | b'-')));
        let end = digits(i + 1 + sign);
        // `2e` with no digits is `2` then the name `e`, which the parser refuses.
        if end > i + 1 + sign {
            i = end;
        }
    }
    i
}

fn number(text: &str, column: usize) -> Result<f32, SyntaxError> {
    match text.parse::<f32>() {
        Ok(n) if n.is_finite() => Ok(n),
        _ => Err(SyntaxError {
            what: format!("the number {text} is out of range"),
            column: Some(column),
            fix: format!("numbers are at most {:e}", f32::MAX),
        }),
    }
}
