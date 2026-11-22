use super::{literals, Line};
use crate::code::{cut, scan, Open};

pub(super) fn split(source: &str) -> Vec<Line> {
    let mut open = Open::None;
    let mut lines: Vec<Line> = source
        .lines()
        .map(|l| {
            let (end, cuts) = scan(l, &mut open, |_, _| {});
            let mut comment = String::new();
            for &(from, to) in &cuts {
                comment.push_str(&l[from..to]);
                comment.push(' ');
            }
            comment.push_str(&l[end..]);
            let code = cut(l, end, &cuts).into_owned();
            Line { code, comment }
        })
        .collect();
    doc_attributes(&mut lines);
    lines
}

fn doc_attributes(lines: &mut [Line]) {
    let code: Vec<&str> = lines.iter().map(|l| l.code.as_str()).collect();
    let code = code.join("\n");
    let found = literals::doc_attributes(&code);
    if found.is_empty() {
        return;
    }
    let line_of = |at: usize| code[..at].matches('\n').count();
    let mut kept = String::new();
    let mut at = 0;
    for (span, strings) in &found {
        kept.push_str(&code[at..span.start]);
        kept.extend(code[span.clone()].chars().filter(|&c| c == '\n'));
        for s in strings {
            let comment = &mut lines[line_of(s.start)].comment;
            comment.push(' ');
            comment.push_str(&code[s.clone()]);
        }
        at = span.end;
    }
    kept.push_str(&code[at..]);
    for (line, code) in lines.iter_mut().zip(kept.split('\n')) {
        line.code = code.to_string();
    }
}
