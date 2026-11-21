//! `comment-cap` and `comment-history` (docs/linting.md, *Comments*), riding the
//! size gate as a signal; `--strict-comments` makes them fail.

mod literals;
mod lua;
pub mod proof;
#[cfg(test)]
mod review_tests;
mod rust;
#[cfg(test)]
mod tests;
mod wgsl;

use crate::agent_docs::history::{reference_as, Refs};
use crate::rules::{allowed_in, Finding};
use std::fs;
use std::path::{Path, PathBuf};

pub const CAP: &str = "comment-cap";
pub const HISTORY: &str = "comment-history";
pub const STRICT: &str = "--strict-comments";

/// A comment opening with one of these costs no words.
const MARKERS: &[&str] = &["lint: allow(", "sorted:", "stored-format:", "comment-cut:"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Rust,
    Lua,
    Wgsl,
}

impl Kind {
    pub fn of(path: &str) -> Option<Kind> {
        match path.rsplit_once('.')?.1 {
            "rs" => Some(Kind::Rust),
            "lua" => Some(Kind::Lua),
            "wgsl" => Some(Kind::Wgsl),
            _ => None,
        }
    }

    pub fn cap(self) -> usize {
        match self {
            Kind::Rust | Kind::Lua => 100,
            Kind::Wgsl => 200,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
    pub code: String,
    pub comment: String,
}

pub fn split(kind: Kind, source: &str) -> Vec<Line> {
    match kind {
        Kind::Rust => rust::split(source),
        Kind::Lua => lua::split(source),
        Kind::Wgsl => wgsl::split(source),
    }
}

fn words(text: &str) -> usize {
    text.split_whitespace()
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .count()
}

fn bare(comment: &str) -> &str {
    comment.trim_start_matches(|c: char| c.is_whitespace() || "/!*-[=".contains(c))
}

/// A comment's opener (`//`, `///`, `//!`, `/*`, `*`).
fn opener(comment: &str) -> &str {
    let comment = comment.trim_start();
    let len = comment.len() - comment.trim_start_matches(['/', '!', '*']).len();
    &comment[..len]
}

/// A marker's line, and a Rust `SAFETY:` note directly above its `unsafe`:
/// the lines from `SAFETY:` on sharing its opener, up to a blank or another.
fn free(kind: Kind, lines: &[Line]) -> Vec<bool> {
    let mut free: Vec<bool> = lines
        .iter()
        .map(|l| MARKERS.iter().any(|m| bare(&l.comment).starts_with(m)))
        .collect();
    let note = |l: &Line| l.code.trim().is_empty() && !l.comment.trim().is_empty();
    for (i, line) in lines.iter().enumerate() {
        if kind != Kind::Rust || !note(line) || !bare(&line.comment).starts_with("SAFETY:") {
            continue;
        }
        let same = |l: &&Line| note(l) && opener(&l.comment) == opener(&line.comment);
        let run = lines[i..].iter().take_while(same).count();
        if lines.get(i + run).is_some_and(|l| opens_unsafe(&l.code)) {
            free[i..i + run].iter_mut().for_each(|f| *f = true);
        }
    }
    free
}

/// `unsafe {`, `unsafe impl`, `unsafe fn`, …
fn opens_unsafe(code: &str) -> bool {
    let mut tokens = Vec::new();
    for piece in code.split(|c: char| c.is_whitespace() || !(crate::code::is_ident(c) || c == '{'))
    {
        tokens.extend(piece.split_inclusive('{').flat_map(|t| {
            let (word, brace) = t.split_at(t.len() - usize::from(t.ends_with('{')));
            [word, brace]
        }));
    }
    tokens.retain(|t| !t.is_empty());
    tokens
        .windows(2)
        .any(|w| w[0] == "unsafe" && matches!(w[1], "{" | "impl" | "fn" | "extern" | "trait"))
}

/// `over`: the line of the first word past the cap, and the words in all.
#[derive(Debug, Default, PartialEq)]
pub struct Report {
    pub over: Option<(usize, usize)>,
    pub history: Vec<(usize, String)>,
}

pub fn check(path: &str, kind: Kind, source: &str) -> Report {
    let lines = split(kind, source);
    let mut report = Report::default();
    let (mut total, mut past) = (0, None);
    for (i, (line, free)) in lines.iter().zip(free(kind, &lines)).enumerate() {
        let what = reference_as(&line.comment, Refs::CODE);
        if let Some(what) = what.filter(|_| !allowed_in(path, source, i + 1, HISTORY)) {
            report.history.push((i + 1, what));
        }
        if free {
            continue;
        }
        total += words(&line.comment);
        if total > kind.cap() && past.is_none() {
            past = Some(i + 1);
        }
    }
    report.over = past.map(|line| (line, total));
    report
}

impl Report {
    pub fn findings(&self, path: &str, kind: Kind) -> Vec<Finding> {
        let at = |line: usize, rule: &'static str, message: String| Finding {
            path: path.to_string(),
            line,
            rule,
            message,
        };
        let cap = self.over.map(|(line, words)| {
            let message = format!(
                "{words} words of comment, cap {}: keep what the code cannot say (a unit, an \
                 invariant, why the obvious way is wrong here) and delete the rest",
                kind.cap()
            );
            at(line, CAP, message)
        });
        let history = self.history.iter().map(|(line, what)| {
            let message = format!(
                "`{what}` is history; say what the code does: `git blame` finds the issue \
                 and the PR"
            );
            at(*line, HISTORY, message)
        });
        cap.into_iter().chain(history).collect()
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct Tally {
    pub files: usize,
    pub over: usize,
    pub words_over: usize,
    pub refs: usize,
    pub with_refs: usize,
}

impl Tally {
    fn add(&mut self, kind: Kind, report: &Report) {
        self.files += 1;
        if let Some((_, words)) = report.over {
            self.over += 1;
            self.words_over += words - kind.cap();
        }
        self.refs += report.history.len();
        self.with_refs += usize::from(!report.history.is_empty());
    }

    pub fn signal(&self) -> String {
        format!(
            "signal (not a failure): {CAP}: {} of {} files over their cap, {} words over; \
             `make comments PATHS=<dir>` lists them\n\
             signal (not a failure): {HISTORY}: {} lines naming an issue or PR, in {} files; \
             `make comments PATHS=<dir>` lists them\n",
            self.over, self.files, self.words_over, self.refs, self.with_refs
        )
    }
}

/// The findings (only when `strict`) and the tally over `files`.
pub fn pass(base: &Path, files: &[PathBuf], strict: bool) -> (Vec<Finding>, Tally) {
    let mut tally = Tally::default();
    let mut found = Vec::new();
    for file in files {
        let rel = file.strip_prefix(base).unwrap_or(file).to_string_lossy();
        let rel = rel.trim_start_matches("./");
        let Some(kind) = Kind::of(rel) else { continue };
        let Ok(source) = fs::read_to_string(file) else {
            continue;
        };
        let report = check(rel, kind, &source);
        tally.add(kind, &report);
        if strict {
            found.extend(report.findings(rel, kind));
        }
    }
    (found, tally)
}
