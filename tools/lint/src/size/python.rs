//! Lines of code in a Python file: what the size gate holds `.py` to (#926).
//!
//! Blank lines, `#` comments and docstrings are free, as scorsese's gate counts
//! them: what grows when a file does too much is its code, while its prose grows
//! with how carefully each part is explained, which the merge scripts do on
//! purpose. A string literal that is data (`X = """…"""`) is code.
//!
//! A docstring is told from data by where it starts: a triple-quoted string that
//! opens a line is a string statement, the shape of every docstring. One limit, by
//! design: a triple-quoted *argument* that opens a line inside a call
//! (`f(\n    """text""")`) reads as free too. Telling the two apart would need the
//! parser; the scripts write no such call.

/// Lines of code in `source`: every line that is not blank, a comment, or inside a
/// docstring. Code after a docstring closes on the same line still counts.
pub(super) fn code_lines(source: &str) -> usize {
    let mut open: Option<Str> = None;
    source
        .lines()
        .filter(|line| is_code(line, &mut open))
        .count()
}

/// A triple-quoted string still open at the end of a line.
#[derive(Clone, Copy)]
struct Str {
    quote: &'static str,
    doc: bool,
}

/// Whether one line counts, given the string open as it starts, and noting the one
/// open as it ends.
fn is_code(line: &str, open: &mut Option<Str>) -> bool {
    let trimmed = line.trim();
    let mut rest = trimmed;
    let mut code = false;
    if open.is_none() {
        if let Some((quote, after)) = opens_triple(trimmed) {
            *open = Some(Str { quote, doc: true });
            rest = after;
        }
    }
    loop {
        if let Some(s) = *open {
            code |= !s.doc && !rest.is_empty();
            match close(rest, s.quote) {
                Some(after) => {
                    *open = None;
                    rest = after;
                }
                None => return code,
            }
        }
        match scan_code(rest) {
            Scan::End(seen) => return code || seen,
            Scan::Opens(seen, quote, after) => {
                code |= seen;
                *open = Some(Str { quote, doc: false });
                rest = after;
            }
        }
    }
}

/// What scanning a stretch of code found.
enum Scan<'a> {
    /// The line ended (or a comment began); whether anything was code.
    End(bool),
    /// A triple-quoted data string opened; what follows its quotes.
    Opens(bool, &'static str, &'a str),
}

/// Scan code up to a comment, the line's end, or a triple quote that opens a
/// string. Single-quoted strings are skipped whole, so a `#` or a quote in one is
/// not mistaken for anything.
fn scan_code(text: &str) -> Scan<'_> {
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut seen = false;
    while i < bytes.len() {
        match bytes[i] {
            b'#' => return Scan::End(seen),
            q @ (b'"' | b'\'') => {
                let triple = if q == b'"' { "\"\"\"" } else { "'''" };
                if bytes[i..].starts_with(triple.as_bytes()) {
                    return Scan::Opens(true, triple, &text[i + 3..]);
                }
                seen = true;
                i = skip_single(bytes, i + 1, q);
                continue;
            }
            b' ' | b'\t' => {}
            _ => seen = true,
        }
        i += 1;
    }
    Scan::End(seen)
}

/// The index past a single-quoted string's closing quote (or the line's end).
fn skip_single(bytes: &[u8], mut i: usize, quote: u8) -> usize {
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b if b == quote => return i + 1,
            _ => i += 1,
        }
    }
    i
}

/// When `line` opens with a triple-quoted string (prefix letters allowed: `r"""`),
/// its quote and what follows the quotes.
fn opens_triple(line: &str) -> Option<(&'static str, &str)> {
    let body = line.trim_start_matches(|c: char| "rRuUbBfF".contains(c));
    if body.len() + 2 < line.len() {
        return None; // more prefix letters than Python allows: an identifier
    }
    ["\"\"\"", "'''"]
        .into_iter()
        .find(|q| body.starts_with(q))
        .map(|q| (q, &body[3..]))
}

/// What follows the closing `quote` in `text`, skipping escaped characters; `None`
/// when the string stays open past this line.
fn close<'a>(text: &'a str, quote: &str) -> Option<&'a str> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
        } else if bytes[i..].starts_with(quote.as_bytes()) {
            return Some(&text[i + 3..]);
        } else {
            i += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::code_lines;

    #[test]
    fn blank_lines_and_comments_are_free() {
        assert_eq!(
            code_lines("\n# a comment\n    # indented\nx = 1  # trailing\n\n"),
            1
        );
    }

    #[test]
    fn docstrings_are_free_one_line_or_many() {
        let src = "\"\"\"Module doc.\"\"\"\n\ndef f():\n    '''Doc,\n    on lines.\n    '''\n    return 1\n";
        assert_eq!(code_lines(src), 2);
        assert_eq!(code_lines("r\"\"\"Raw doc.\"\"\"\nx = 1\n"), 1);
    }

    #[test]
    fn a_string_that_is_data_is_code() {
        // Every line of the assigned string counts; its closing quote is not taken
        // for a docstring opening, which would swallow the code after it.
        let src = "X = \"\"\"\na\nb\n\"\"\"\ny = 2\nz = 3\n";
        assert_eq!(code_lines(src), 6);
    }

    #[test]
    fn code_after_a_docstring_closes_counts() {
        assert_eq!(code_lines("\"\"\"doc\"\"\"; x = 1\n"), 1);
        assert_eq!(code_lines("\"\"\"doc\n\"\"\" # only a comment\n"), 0);
    }

    #[test]
    fn hashes_and_quotes_inside_strings_are_not_comments_or_docstrings() {
        assert_eq!(code_lines("x = \"# not a comment\"\n"), 1);
        assert_eq!(code_lines("x = 'it\\'s'  # comment\ny = 1\n"), 2);
        let escaped = "X = \"\"\"a \\\"\"\" still open\n\"\"\"\n";
        assert_eq!(code_lines(escaped), 2);
    }

    #[test]
    fn text_beyond_ascii_is_read_whole() {
        // A byte index inside `—` once panicked the scan: quotes are matched on bytes.
        assert_eq!(code_lines("\"\"\"a — b\n— c \\\" —\"\"\"\nx = '—'\n"), 1);
    }

    #[test]
    fn an_identifier_starting_with_prefix_letters_is_code() {
        assert_eq!(code_lines("buf = 1\nreturn x\n"), 2);
    }
}
