//! Source scan for the layering lint: which top-level modules each file names.
//!
//! Test code is exempt (#724), as `gate` defines it: a test-gated item is
//! skipped here, and the graph drops the test-only files whole. Not a parser — a
//! coarse line scan, like the direction guard.

use super::extent::{item_end, strip_comment, Open};
use super::gate::{is_test_gate, test_only_files};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// The tree under `src`, read once and shared by the module list, the edge scan
/// and the external-crate scan (#1136): every `.rs` file's text, and which files
/// are test-only.
pub struct Tree {
    pub src: PathBuf,
    sources: BTreeMap<PathBuf, String>,
    test_only: BTreeSet<PathBuf>,
}

impl Tree {
    pub fn read(src: &Path) -> Self {
        let files = walk(src);
        let sources: BTreeMap<PathBuf, String> = files
            .iter()
            .map(|f| (f.clone(), fs::read_to_string(f).unwrap_or_default()))
            .collect();
        let test_only = test_only_files(&files, |f| sources.get(f).map(String::as_str));
        Self {
            src: src.to_path_buf(),
            sources,
            test_only,
        }
    }

    pub fn is_test_only(&self, path: &Path) -> bool {
        self.test_only.contains(path)
    }

    /// Each file that is not test-only, with its text, in path order.
    pub fn live_files(&self) -> impl Iterator<Item = (&Path, &str)> {
        self.sources
            .iter()
            .filter(|(f, _)| !self.is_test_only(f))
            .map(|(f, text)| (f.as_path(), text.as_str()))
    }
}

/// One `crate::<module>` reference in non-test, non-comment code.
pub struct Ref {
    pub line: usize,
    pub module: String,
}

/// Every `.rs` file under `dir`, sorted so reports are stable.
pub fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk_into(dir, &mut out);
    out.sort();
    out
}

fn walk_into(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_into(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

pub fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The `crate::<module>` references in `content`, minus comments and inline
/// `#[cfg(test)]` items. `crate::{a, b::C}` yields both `a` and `b`.
pub fn refs(content: &str) -> Vec<Ref> {
    let mut out = Vec::new();
    let mut group: Option<i32> = None;
    for (line, code) in live_lines(content) {
        for module in crate_refs(code, &mut group) {
            out.push(Ref { line, module });
        }
    }
    out
}

/// The lines of `content` that are live (non-test) code, numbered from 1, each with
/// its `//` comment stripped (a string open across lines carried, so a `//` inside
/// it is kept and one after it is cut). Shared with the external-crate scan.
pub fn live_lines(content: &str) -> Vec<(usize, &str)> {
    let lines: Vec<&str> = content.lines().collect();
    let mut out = Vec::new();
    let (mut i, mut open) = (0, Open::None);
    while i < lines.len() {
        let inside = open != Open::None;
        let code = strip_comment(lines[i], &mut open);
        if !inside && is_test_gate(&lines[i..]) {
            i = item_end(&lines, i) + 1;
            open = Open::None;
            continue;
        }
        out.push((i + 1, code));
        i += 1;
    }
    out
}

/// Module names a line references. `group` carries an open `crate::{` across
/// lines: `Some(depth)` while inside it, so a multi-line import still counts.
fn crate_refs(code: &str, group: &mut Option<i32>) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = code;
    if let Some(d) = *group {
        if d == 1 {
            out.extend(leading_ident(rest));
        }
        rest = group_names(rest, group, &mut out);
    }
    while let Some(at) = find_crate(rest) {
        let after = &rest[at + "crate::".len()..];
        if let Some(inner) = after.strip_prefix('{') {
            *group = Some(1);
            out.extend(leading_ident(inner));
            rest = group_names(inner, group, &mut out);
        } else {
            out.extend(leading_ident(after));
            rest = after;
        }
    }
    out
}

/// Walk an open `crate::{ … }` group, collecting the first segment of each entry
/// at depth 1; returns what follows the group's close (or "" if still open).
fn group_names<'a>(s: &'a str, group: &mut Option<i32>, out: &mut Vec<String>) -> &'a str {
    let Some(mut d) = *group else { return s };
    for (at, c) in s.char_indices() {
        match c {
            '{' => d += 1,
            '}' if d == 1 => {
                *group = None;
                return &s[at + 1..];
            }
            '}' => d -= 1,
            ',' if d == 1 => out.extend(leading_ident(&s[at + 1..])),
            _ => {}
        }
    }
    *group = Some(d);
    ""
}

fn find_crate(s: &str) -> Option<usize> {
    s.match_indices("crate::")
        .find(|(at, _)| !s[..*at].chars().next_back().is_some_and(is_ident))
        .map(|(at, _)| at)
}

fn leading_ident(s: &str) -> Option<String> {
    let s = s.trim_start();
    let end = s.find(|c: char| !is_ident(c)).unwrap_or(s.len());
    (end > 0 && s[..end] != *"self").then(|| s[..end].to_string())
}

#[cfg(test)]
#[path = "scan_tests.rs"]
mod tests;
