use super::{bare, marked, split, Kind};
use crate::rules::{allowed_in, Finding};
use std::fs;
use std::path::Path;

pub const WIDTH: &str = "comment-width";

/// rustfmt's `max_width`, read from the repository's `rustfmt.toml`; rustfmt's default without
/// it. A value that is not a width is a finding on its line, never a silent default.
pub fn max_width(base: &Path) -> Result<usize, Finding> {
    let toml = fs::read_to_string(base.join("rustfmt.toml")).unwrap_or_default();
    let Some((i, value)) = toml.lines().enumerate().find_map(|(i, l)| {
        let (key, value) = l.split_once('=')?;
        (key.trim() == "max_width").then_some((i, value))
    }) else {
        return Ok(100);
    };
    let value = value.split_once('#').map_or(value, |(v, _)| v).trim();
    value.parse().map_err(|_| Finding {
        path: "rustfmt.toml".to_string(),
        line: i + 1,
        rule: WIDTH,
        message: format!("`max_width = {value}` is not a width: write a whole number of columns"),
    })
}

pub fn check(path: &str, kind: Kind, source: &str, max: usize) -> Vec<Finding> {
    let mut found = Vec::new();
    for (i, (text, line)) in source.lines().zip(split(kind, source)).enumerate() {
        let len = text.chars().count();
        let comment = line.comment.trim();
        if len <= max || comment.is_empty() || marked(comment) {
            continue;
        }
        if unbreakable(text, comment, max) || allowed_in(path, source, i + 1, WIDTH) {
            continue;
        }
        found.push(Finding {
            path: path.to_string(),
            line: i + 1,
            rule: WIDTH,
            message: format!(
                "comment line of {len} columns, width {max} (`rustfmt.toml`'s `max_width`): \
                 wrap it at the width, the same words on more lines"
            ),
        });
    }
    found
}

/// The excess is one token that would not fit even on a comment line of its own:
/// wrapping cannot bring the line inside the width.
fn unbreakable(text: &str, comment: &str, max: usize) -> bool {
    let chars: Vec<char> = text.chars().collect();
    if chars[max..].iter().any(|c| c.is_whitespace()) {
        return false;
    }
    let start = chars[..max]
        .iter()
        .rposition(|c| c.is_whitespace())
        .map_or(0, |at| at + 1);
    let indent = chars.iter().take_while(|c| c.is_whitespace()).count();
    let opener = comment.len() - bare(comment).len();
    indent + opener + (chars.len() - start) > max
}
