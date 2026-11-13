//! `stored-format`: a module that keeps a serde type in a file says so (#1966).
//!
//! `make review-level` (`.github/scripts/dispatch/review_level.py`) reviews a
//! change to an on-disk format at `high`. It used to learn which modules those are
//! from a hand-kept path table, and missed a new format three times in one batch
//! (#1946's hull sidecars, #1942's `.animgraph`, PR #1962's `.occlusion`): each new
//! format is a module the table had not heard of. Now the module declares it where
//! it lives, in its top doc comment:
//!
//! ```text
//! //! stored-format: the `<scene>.occlusion` file beside the scene
//! ```
//!
//! A marker in `mod.rs` covers its directory and everything under it; one in
//! `x.rs` covers that file and `x/`. `review_level.py` reads the markers.
//!
//! This rule keeps the marker from being forgotten. A non-test file of `src/` is a
//! finding, on every file read or write, when it
//! * reads or writes a file (`fs::write`, `fs::read`, `fs::read_to_string`,
//!   `File::open`, `File::create`, `OpenOptions`, `write_atomic`, and
//!   `written::write`, which every engine asset write goes through), and
//! * either encodes or decodes with a serde format crate (`serde_json::to_*` or
//!   `serde_json::from_*`), wherever the type is derived, or names serde
//!   (`serde_json`, `serde::`) in a directory whose non-test code derives
//!   `Serialize` or `Deserialize`,
//!
//! and no marker covers it. Every IO line is reported, so an allow on one never
//! excuses another. A file that uses serde and touches disk for another reason
//! (it reads a script, not a document) says so with
//! `// lint: allow(stored-format) <why>` on each such line. A format coded by hand (the
//! binary `.occlusion` and `.navmesh` files, a KTX2 cubemap) never trips the rule:
//! its module is marked by hand. A marker with no text after the colon is a finding.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::{code_lines, is_ident};

pub const RULE: &str = "stored-format";
pub const MARKER: &str = "//! stored-format:";

/// What reads or writes a file. Each must stand alone as a word at its end, so
/// `fs::read` is not `fs::read_dir`.
const IO: &[&str] = &[
    "fs::write",
    "fs::read",
    "fs::read_to_string",
    "File::open",
    "File::create",
    "OpenOptions",
    "write_atomic",
    "written::write",
];

/// A serde format crate's encode or decode call: the type it moves may be
/// derived anywhere, so these say "a document" on their own.
const CODEC: &[&str] = &["serde_json::to_", "serde_json::from_"];

/// One scanned source: its path, its text, and which lines are test code.
pub struct Source<'a> {
    pub path: &'a str,
    pub text: &'a str,
    pub test: &'a [bool],
}

fn ends_word(code: &str, token: &str) -> bool {
    code.match_indices(token).any(|(at, _)| {
        let before = code[..at].chars().next_back();
        let after = code[at + token.len()..].chars().next();
        !before.is_some_and(is_ident) && !after.is_some_and(is_ident)
    })
}

/// The code lines of `s` that are not test code, numbered from 1.
fn live<'a>(s: &'a Source) -> Vec<(usize, std::borrow::Cow<'a, str>)> {
    code_lines(s.text)
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !s.test.get(*i).copied().unwrap_or(false))
        .map(|(i, c)| (i + 1, c))
        .collect()
}

/// The code lines joined back into one text, so a construct may span lines.
fn joined(code: &[(usize, std::borrow::Cow<str>)]) -> String {
    code.iter()
        .map(|(_, c)| c.as_ref())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether any `derive(...)` in `code` names `Serialize` or `Deserialize`, also
/// when rustfmt wraps the list over several lines.
fn derives_serde(code: &str) -> bool {
    code.match_indices("derive(").any(|(at, open)| {
        let rest = &code[at + open.len()..];
        let list = &rest[..rest.find(')').unwrap_or(rest.len())];
        list.contains("Serialize") || list.contains("Deserialize")
    })
}

/// The text of the file's marker, if it has one (empty when it says nothing).
pub fn marker(text: &str) -> Option<(usize, &str)> {
    text.lines().enumerate().find_map(|(i, l)| {
        l.trim_start()
            .strip_prefix(MARKER)
            .map(|t| (i + 1, t.trim()))
    })
}

/// Whether a marked file covers `path`: the file itself, a `mod.rs` of its
/// directory or one above it, or the `<dir>.rs` beside any of those directories.
pub fn covered(path: &str, marked: &[&str]) -> bool {
    if marked.contains(&path) {
        return true;
    }
    let mut dir = path;
    while let Some((parent, _)) = dir.rsplit_once('/') {
        let owners = [format!("{parent}/mod.rs"), format!("{parent}.rs")];
        if owners.iter().any(|o| marked.contains(&o.as_str())) {
            return true;
        }
        dir = parent;
    }
    false
}

/// Every finding over `sources`.
pub fn check(sources: &[Source]) -> Vec<Finding> {
    let mut found = Vec::new();
    let mut marked = Vec::new();
    for s in sources {
        match marker(s.text) {
            Some((line, "")) => found.push(finding(
                s.path,
                line,
                "a stored-format marker names the file it keeps",
            )),
            Some(_) => marked.push(s.path),
            None => {}
        }
    }
    let engine = |s: &&Source| crate::modules::module_path(s.path).starts_with("src/");
    let lives: Vec<_> = sources
        .iter()
        .filter(engine)
        .map(|s| (s, live(s)))
        .collect();
    let serde_dirs: Vec<&str> = lives
        .iter()
        .filter(|(_, code)| derives_serde(&joined(code)))
        .filter_map(|(s, _)| s.path.rsplit_once('/').map(|(d, _)| d))
        .collect();
    for (s, code) in &lives {
        let dir = s.path.rsplit_once('/').map_or("", |(d, _)| d);
        let any = |p: &dyn Fn(&str) -> bool| code.iter().any(|(_, c)| p(c));
        let calls = any(&|c| CODEC.iter().any(|t| c.contains(t)));
        let names = any(&|c| c.contains("serde_json") || c.contains("serde::"));
        let kept = calls || (names && serde_dirs.contains(&dir));
        if !kept || covered(s.path, &marked) {
            continue;
        }
        let io = code
            .iter()
            .filter(|(_, c)| IO.iter().any(|t| ends_word(c, t)));
        for (line, _) in io {
            let why =
                "a serde type kept in a file: mark its module `//! stored-format: <the file>` \
                       so `make review-level` reads it as a stored format";
            found.push(finding(s.path, *line, why));
        }
    }
    found
}

fn finding(path: &str, line: usize, message: &str) -> Finding {
    Finding {
        path: path.to_string(),
        line,
        rule: RULE,
        message: message.to_string(),
    }
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    let sources: Vec<Source> = scan
        .files
        .iter()
        .map(|p| Source {
            path: p,
            text: scan.source(p),
            test: scan.test_lines(p),
        })
        .collect();
    check(&sources)
}

#[cfg(test)]
#[path = "stored_format_tests.rs"]
mod tests;
