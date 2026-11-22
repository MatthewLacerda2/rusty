pub(super) fn code_lines(source: &str) -> usize {
    let mut open: Option<Long> = None;
    source
        .lines()
        .filter(|line| is_code(line.as_bytes(), &mut open))
        .count()
}

#[derive(Clone, Copy)]
struct Long {
    level: usize,
    comment: bool,
}

fn is_code(bytes: &[u8], open: &mut Option<Long>) -> bool {
    let mut code = false;
    let mut i = 0;
    while i < bytes.len() {
        if let Some(long) = *open {
            let (end, closed) = close(bytes, i, long.level);
            code |= !long.comment && bytes[i..end].iter().any(|b| !b.is_ascii_whitespace());
            if !closed {
                return code;
            }
            *open = None;
            i = end;
            continue;
        }
        match bytes[i] {
            b'-' if bytes.get(i + 1) == Some(&b'-') => {
                match long_open(bytes, i + 2) {
                    Some((level, after)) => {
                        *open = Some(Long {
                            level,
                            comment: true,
                        });
                        i = after;
                    }
                    None => return code,
                }
                continue;
            }
            b'[' => {
                if let Some((level, after)) = long_open(bytes, i) {
                    *open = Some(Long {
                        level,
                        comment: false,
                    });
                    code = true;
                    i = after;
                    continue;
                }
                code = true;
            }
            q @ (b'"' | b'\'') => {
                code = true;
                i = skip_quoted(bytes, i + 1, q);
                continue;
            }
            b if b.is_ascii_whitespace() => {}
            _ => code = true,
        }
        i += 1;
    }
    code
}

fn long_open(bytes: &[u8], i: usize) -> Option<(usize, usize)> {
    if bytes.get(i) != Some(&b'[') {
        return None;
    }
    let level = bytes[i + 1..].iter().take_while(|&&b| b == b'=').count();
    (bytes.get(i + 1 + level) == Some(&b'[')).then_some((level, i + 2 + level))
}

fn close(bytes: &[u8], mut i: usize, level: usize) -> (usize, bool) {
    while i < bytes.len() {
        if bytes[i] == b']' {
            let eq = bytes[i + 1..].iter().take_while(|&&b| b == b'=').count();
            if eq == level && bytes.get(i + 1 + eq) == Some(&b']') {
                return (i + 2 + eq, true);
            }
        }
        i += 1;
    }
    (bytes.len(), false)
}

fn skip_quoted(bytes: &[u8], mut i: usize, quote: u8) -> usize {
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b if b == quote => return i + 1,
            _ => i += 1,
        }
    }
    bytes.len()
}

#[cfg(test)]
mod tests {
    use super::code_lines;

    #[test]
    fn blank_lines_and_comments_are_free() {
        assert_eq!(
            code_lines("\n-- a comment\n  -- indented\nx = 1 -- trailing\n\n"),
            1
        );
    }

    #[test]
    fn long_comments_are_free_at_any_level() {
        assert_eq!(code_lines("--[[ one ]]\n--[[\nmany\n]]\nx = 1\n"), 1);
        assert_eq!(code_lines("--[==[\n]] not closed yet\n]==]\nx = 1\n"), 1);
        assert_eq!(code_lines("--[[ doc ]] x = 1\n"), 1);
    }

    #[test]
    fn a_long_string_is_code_on_every_line() {
        // The blank line inside is blank either way; the closing `]]` line is code.
        let src = "local s = [[\n-- not a comment\n\n]]\nx = 1\n";
        assert_eq!(code_lines(src), 4);
    }

    #[test]
    fn dashes_inside_quoted_strings_are_not_comments() {
        assert_eq!(code_lines("x = \"-- text\"\ny = 'it\\'s -- x'\n"), 2);
        assert_eq!(code_lines("t[1] = 2 -- comment\n"), 1);
    }

    #[test]
    fn text_beyond_ascii_is_read_whole() {
        assert_eq!(code_lines("-- — é\nx = '—' -- —\n--[[ é ]]\n"), 1);
    }
}
