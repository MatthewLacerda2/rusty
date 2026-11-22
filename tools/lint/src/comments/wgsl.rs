use super::Line;

pub(super) fn split(source: &str) -> Vec<Line> {
    let mut depth = 0usize;
    source.lines().map(|l| line(l, &mut depth)).collect()
}

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
