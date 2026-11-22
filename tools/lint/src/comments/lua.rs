use super::{literals, Line};

#[derive(Clone, Copy)]
struct Long {
    level: usize,
    comment: bool,
}

pub(super) fn split(source: &str) -> Vec<Line> {
    let mut open = None;
    source.lines().map(|l| line(l, &mut open)).collect()
}

fn line(mut rest: &str, open: &mut Option<Long>) -> Line {
    let mut out = Line::default();
    while !rest.is_empty() {
        if let Some(long) = *open {
            let end = literals::long_close(rest, long.level);
            let to = if long.comment {
                &mut out.comment
            } else {
                &mut out.code
            };
            to.push_str(&rest[..end.unwrap_or(rest.len())]);
            let Some(end) = end else { break };
            if long.comment {
                out.code.push(' ');
            }
            *open = None;
            rest = &rest[end..];
        } else if let Some(after) = rest.strip_prefix("--") {
            match literals::long_open(after) {
                Some((level, len)) => {
                    out.comment.push_str(&rest[..2 + len]);
                    *open = Some(Long {
                        level,
                        comment: true,
                    });
                    rest = &after[len..];
                }
                None => {
                    out.comment.push_str(rest);
                    break;
                }
            }
        } else if let Some((level, len)) = literals::long_open(rest) {
            out.code.push_str(&rest[..len]);
            *open = Some(Long {
                level,
                comment: false,
            });
            rest = &rest[len..];
        } else {
            let c = rest.chars().next().unwrap_or(' ');
            let len = literals::quoted(rest).unwrap_or(c.len_utf8());
            out.code.push_str(&rest[..len]);
            rest = &rest[len..];
        }
    }
    out
}
