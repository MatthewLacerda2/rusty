use super::{bare, Kind, Line};
use crate::rules::{allowed_in, Finding};

pub const PATH: &str = "comment-path";
pub const ECHO: &str = "comment-echo";

/// Words that carry no claim: a line made of them and the module's own names says nothing.
const FILLER: &[&str] = &[
    "a", "all", "an", "and", "any", "are", "as", "at", "be", "by", "each", "every", "for", "from",
    "in", "into", "is", "it", "its", "of", "on", "one", "or", "see", "that", "the", "their",
    "them", "these", "this", "those", "to", "with",
];

pub fn check(path: &str, kind: Kind, source: &str, lines: &[Line]) -> Vec<Finding> {
    let at = |line: usize, rule: &'static str, message: String| Finding {
        path: path.to_string(),
        line,
        rule,
        message,
    };
    let mut found = Vec::new();
    if let Some((i, word)) = path_prefix(path, lines) {
        let message = format!(
            "the comment opens with `{word}`, the file's own path: start with what the \
             file holds; the path is already the file's"
        );
        found.push(at(i + 1, PATH, message));
    }
    if let Some(i) = echoes(path, lines).filter(|_| kind == Kind::Rust) {
        let message = "this `//!` only names its module (every word is in the path or an \
                       item's name): state what they cannot, an order, a unit or a rule, or \
                       delete the line"
            .to_string();
        found.push(at(i + 1, ECHO, message));
    }
    found.retain(|f| !allowed_in(path, source, f.line, f.rule));
    found
}

fn path_prefix<'a>(path: &str, lines: &'a [Line]) -> Option<(usize, &'a str)> {
    let (i, line) = lines
        .iter()
        .enumerate()
        .find(|(_, l)| !bare(&l.comment).trim().is_empty())?;
    let first = bare(&line.comment).split_whitespace().next()?;
    let word = first.trim_matches(|c: char| "`*:,;—".contains(c));
    let tail = path == word || path.ends_with(&format!("/{word}"));
    (word.contains('.') && tail).then_some((i, word))
}

/// The line of a one-line `//!` whose every content word is the path's or a declared name's.
fn echoes(path: &str, lines: &[Line]) -> Option<usize> {
    let inner = |l: &Line| l.code.trim().is_empty() && l.comment.trim_start().starts_with("//!");
    let start = lines
        .iter()
        .take_while(|l| l.code.trim().is_empty() && !inner(l))
        .count();
    let header = lines[start..].iter().take_while(|l| inner(l)).count();
    if header != 1 {
        return None;
    }
    let text = bare(&lines[start].comment);
    let pointer = text.split('`').skip(1).step_by(2).any(is_doc_path);
    let prose: String = text
        .split('`')
        .enumerate()
        .filter(|(i, span)| i % 2 == 0 || !is_doc_path(span))
        .map(|(_, span)| span)
        .collect::<Vec<_>>()
        .join(" ");
    let content = words(&prose)
        .into_iter()
        .filter(|w| !FILLER.contains(&w.as_str()))
        .collect::<Vec<_>>();
    if content.is_empty() {
        return pointer.then_some(start);
    }
    let mut known = words(path);
    for line in lines {
        let names = declared(&line.code).map(|(_, name)| words(name));
        known.extend(names.filter(|w| w.len() <= NAME_WORDS).flatten());
    }
    let echo = content.iter().all(|w| known.iter().any(|k| same(w, k)));
    echo.then_some(start)
}

fn is_doc_path(span: &str) -> bool {
    span.starts_with("docs/") || span.ends_with(".md")
}

/// The lowercase words of a text or a name: `snake_case`, `CamelCase`, a path's segments.
pub(super) fn words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for run in text.split(|c: char| !c.is_alphanumeric() && c != '\'') {
        let run = run.trim_matches('\'');
        let run = run.strip_suffix("'s").unwrap_or(run);
        let mut word = String::new();
        let mut prev_lower = false;
        for c in run.chars().filter(|&c| c != '\'') {
            if c.is_uppercase() && prev_lower {
                out.push(std::mem::take(&mut word));
            }
            prev_lower = c.is_lowercase() || c.is_ascii_digit();
            word.extend(c.to_lowercase());
        }
        out.push(word);
    }
    out.retain(|w| !w.is_empty());
    out
}

/// One word, or the same word in the plural: `stage`/`stages`, `box`/`boxes`, `body`/`bodies`.
fn same(word: &str, known: &str) -> bool {
    let plural_of = |p: &str, s: &str| {
        let sibilant = ["s", "x", "z", "ch", "sh"].iter().any(|e| s.ends_with(e));
        let y = s.strip_suffix('y').map(|stem| format!("{stem}ies"));
        p.strip_suffix('s') == Some(s)
            || sibilant && p.strip_suffix("es") == Some(s)
            || y.is_some_and(|y| p == y)
    };
    word == known || plural_of(word, known) || plural_of(known, word)
}

/// A longer name is a sentence (a test's), which would let any line echo it.
const NAME_WORDS: usize = 3;

/// The names `code` declares, each with the keyword that declares it.
pub(crate) fn declared(code: &str) -> impl Iterator<Item = (&str, &str)> {
    let tokens: Vec<&str> = code
        .split(|c: char| !crate::code::is_ident(c))
        .filter(|t| !t.is_empty())
        .collect();
    let named: Vec<(&str, &str)> = tokens
        .windows(2)
        .filter(|w| crate::code::ITEMS.contains(&w[0]))
        .map(|w| (w[0], w[1]))
        .collect();
    named.into_iter()
}

#[cfg(test)]
#[path = "header_tests.rs"]
mod tests;
