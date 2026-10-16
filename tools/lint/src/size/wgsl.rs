//! Lines of code in a WGSL shader: what the size gate holds `.wgsl` to (#1015).
//!
//! Blank lines and comments are free, as for Python: a shader's comments explain
//! its maths, and a carefully explained shader is not a long one. WGSL has `//`
//! line comments and `/* … */` block comments, which **nest**; it has no string
//! literals, so nothing else can hide a comment marker.

/// Lines of code in `source`: every line holding something outside a comment.
pub(super) fn code_lines(source: &str) -> usize {
    let mut depth = 0usize;
    source
        .lines()
        .filter(|line| is_code(line.as_bytes(), &mut depth))
        .count()
}

/// Whether one line counts, given the block-comment depth it starts at, and
/// noting the depth it ends at.
fn is_code(bytes: &[u8], depth: &mut usize) -> bool {
    let mut code = false;
    let mut i = 0;
    while i < bytes.len() {
        let pair = &bytes[i..bytes.len().min(i + 2)];
        if pair == b"/*" {
            *depth += 1;
            i += 2;
        } else if *depth > 0 && pair == b"*/" {
            *depth -= 1;
            i += 2;
        } else if *depth == 0 && pair == b"//" {
            break;
        } else {
            code |= *depth == 0 && !bytes[i].is_ascii_whitespace();
            i += 1;
        }
    }
    code
}

#[cfg(test)]
mod tests {
    use super::code_lines;

    #[test]
    fn blank_lines_and_line_comments_are_free() {
        let src = "\n// a comment\n    // indented\nlet x = 1.0; // trailing\n\n";
        assert_eq!(code_lines(src), 1);
    }

    #[test]
    fn block_comments_are_free_on_one_line_or_many() {
        assert_eq!(code_lines("/* one */\n/*\n many\n*/\nfn f() {}\n"), 1);
        assert_eq!(code_lines("let a = /* inline */ 1.0;\n"), 1);
        assert_eq!(code_lines("/* doc */ let a = 1.0;\n"), 1);
    }

    #[test]
    fn block_comments_nest() {
        // A naive scan closes at the inner `*/` and counts `still comment` as code.
        let src = "/* outer\n/* inner */\nstill comment\n*/\nlet a = 1;\n";
        assert_eq!(code_lines(src), 1);
    }

    #[test]
    fn a_line_comment_marker_inside_a_block_comment_closes_nothing() {
        assert_eq!(code_lines("/* a // b\n*/ let x = 1;\n"), 1);
    }

    #[test]
    fn text_beyond_ascii_is_read_whole() {
        assert_eq!(code_lines("// θ — φ\nlet θ = 1.0; /* — */\n"), 1);
    }
}
