//! A WGSL shader split into code and comment: `//` to the line's end, and
//! `/* … */`, which **nests**. WGSL has no string literals, so nothing else can
//! hide a comment marker.

use super::Line;

/// Every line of `source`, its comment apart from its code.
pub(super) fn split(source: &str) -> Vec<Line> {
    let mut depth = 0usize;
    source.lines().map(|l| line(l, &mut depth)).collect()
}

/// One line, starting `depth` block comments deep and leaving the depth it ends
/// at. A block comment that closes leaves a space in the code, so the tokens
/// either side of it stay apart.
fn line(mut rest: &str, depth: &mut usize) -> Line {
    let mut out = Line::default();
    while let Some(c) = rest.chars().next() {
        if rest.starts_with("/*") {
            *depth += 1;
            out.comment.push_str("/*");
            rest = &rest[2..];
        } else if *depth > 0 && rest.starts_with("*/") {
            *depth -= 1;
            out.comment.push_str("*/ ");
            if *depth == 0 {
                out.code.push(' ');
            }
            rest = &rest[2..];
        } else if *depth == 0 && rest.starts_with("//") {
            out.comment.push_str(rest);
            break;
        } else {
            let to = if *depth > 0 {
                &mut out.comment
            } else {
                &mut out.code
            };
            to.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}
