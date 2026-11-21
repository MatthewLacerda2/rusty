//! `comment-cap` and `comment-history`: what the comments in code may hold.
//!
//! Every agent that reads a file pays for its comments, so a file's comments hold
//! at most [`Kind::cap`] words, and none names one of this repository's issues
//! or pull requests: the code says what it does, and `git blame` finds the story.
//!
//! A comment is Rust's `//`, `///`, `//!`, `/* */` and the string of a doc
//! attribute ([`rust`]), WGSL's `//` and `/* */` ([`wgsl`]) and Lua's `--` and
//! `--[[ ]]` ([`lua`]), at a line's end too. A word is a whitespace-separated
//! run holding a letter or digit, so a comment's markers and rulers are free.
//! Free too: the line of a marker a tool reads ([`MARKERS`]), and a `SAFETY:`
//! note directly above the `unsafe` it justifies.
//!
//! They ride the size gate over the files it measures, as a signal: one line
//! each under `signal (not a failure)`, unless `--strict-comments` names paths,
//! whose findings then fail ([`pass`]). The same split proves a diff touches
//! only comments ([`proof`]).

mod literals;
mod lua;
pub mod proof;
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
/// The size gate's flag that turns these checks' findings under the paths after
/// it into failures.
pub const STRICT: &str = "--strict-comments";

/// The markers a tool reads, each free on its own line: a comment that starts
/// with one costs no words.
const MARKERS: &[&str] = &["lint: allow(", "sorted:", "stored-format:", "comment-cut:"];

/// The languages whose comments are held.
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

    /// The words of comment one file may hold: a shader's maths earns twice.
    pub fn cap(self) -> usize {
        match self {
            Kind::Rust | Kind::Lua => 100,
            Kind::Wgsl => 200,
        }
    }
}

/// One line of source: its code, comments cut, and its comment text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
    pub code: String,
    pub comment: String,
}

/// Every line of `source`, read as `kind`.
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

/// A comment's text with its markers and decoration laid aside.
fn bare(comment: &str) -> &str {
    comment.trim_start_matches(|c: char| c.is_whitespace() || "/!*-[=".contains(c))
}

/// Which lines' comments are free: a marker's line, and in Rust each line of a
/// `SAFETY:` note directly above a line of code holding `unsafe`.
fn free(kind: Kind, lines: &[Line]) -> Vec<bool> {
    let mut free: Vec<bool> = lines
        .iter()
        .map(|l| MARKERS.iter().any(|m| bare(&l.comment).starts_with(m)))
        .collect();
    let note = |l: &Line| l.code.trim().is_empty();
    for (i, line) in lines.iter().enumerate() {
        if kind != Kind::Rust || !note(line) || !bare(&line.comment).starts_with("SAFETY:") {
            continue;
        }
        let run = lines[i..].iter().take_while(|l| note(l)).count();
        let below = lines.get(i + run).map_or("", |l| l.code.as_str());
        if below
            .split(|c: char| !crate::code::is_ident(c))
            .any(|w| w == "unsafe")
        {
            free[i..i + run].iter_mut().for_each(|f| *f = true);
        }
    }
    free
}

/// What one file's comments break: the cap (the line its first word past the cap
/// sits on, and the words it holds) and each line naming an issue or PR.
#[derive(Debug, Default, PartialEq)]
pub struct Report {
    pub over: Option<(usize, usize)>,
    pub history: Vec<(usize, String)>,
}

/// The report for the file at repo-relative `path`, its text `source`.
pub fn check(path: &str, kind: Kind, source: &str) -> Report {
    let lines = split(kind, source);
    let mut report = Report::default();
    let (mut total, mut past) = (0, None);
    for (i, (line, free)) in lines.iter().zip(free(kind, &lines)).enumerate() {
        if free {
            continue;
        }
        total += words(&line.comment);
        if total > kind.cap() && past.is_none() {
            past = Some(i + 1);
        }
        let what = reference_as(&line.comment, Refs::CODE);
        if let Some(what) = what.filter(|_| !allowed_in(path, source, i + 1, HISTORY)) {
            report.history.push((i + 1, what));
        }
    }
    report.over = past.map(|line| (line, total));
    report
}

impl Report {
    /// Its findings, in the one report shape.
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

/// The totals the signal prints.
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

    /// The two signal lines.
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

/// The checks over `files` (on disk, under `base`): the findings under `strict`'s
/// paths, which fail, and the tally of all of them, which only prints.
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
