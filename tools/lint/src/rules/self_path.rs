//! `self-path`: a header that opens with a path names its own file (#1654).
//!
//! `//! src/scene/prefab/link.rs — …` is a promise about where the file is. The
//! directory regroupings `dir-size` drives (#1367) move files and leave the header
//! behind, and a stale path sends an agent to a place that is not there (the #1650
//! audit fixed about 35 by hand). When a source file's first module-doc line
//! (`//!`) or a shader's first line (`// …`) opens with a path under a repository
//! root (`src/`, `crates/`, `tests/`, `tools/`, `fuzz/`, `engine/`, `.github/`), that path
//! must be the file's own. A `mod.rs` may instead name its directory, `src/a/b/`
//! or `src/a/b/mod.rs`. A word counts as a path when it is shaped like one (a
//! directory with its trailing `/`, a file with its extension) or names a directory
//! holding scanned files, at its real or its module path (`tools/lint`, `src/core`
//! once `core` is a crate): a bare directory header goes stale like any other. A
//! word that names no such directory is prose. A header that opens with no path,
//! or a file with no header, is left alone: the rule never asks for one. A header that names another
//! file on purpose says why with `// lint: allow(self-path) <why>` on the line.
//!
//! The rule reads `engine/**/*.wgsl` itself, as `fullscreen-triangle` does, since
//! `Scan` holds only Rust.

use super::finding::{self, Finding};
use super::scan::Scan;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub const RULE: &str = "self-path";

/// What a header's leading path starts with to count as a path.
const ROOTS: &[&str] = &[
    "src/", "crates/", "tests/", "tools/", "fuzz/", "engine/", ".github/",
];

/// How many leading lines are searched for the header (attributes may come first).
const HEAD: usize = 10;

/// Every directory holding a scanned file, at its real path and its module path
/// (`crate::modules`): what a bare header word must name to count as a path.
pub fn dirs_of<'a>(files: impl IntoIterator<Item = &'a String>) -> BTreeSet<String> {
    let mut dirs = BTreeSet::new();
    for file in files {
        for path in [file.clone(), crate::modules::module_path(file).into_owned()] {
            let mut at = path.as_str();
            while let Some((dir, _)) = at.rsplit_once('/') {
                dirs.insert(dir.to_string());
                at = dir;
            }
        }
    }
    dirs
}

/// The header's leading path, if it opens with one: the first word, without
/// backticks or trailing punctuation.
fn leading_path<'a>(text: &'a str, dirs: &BTreeSet<String>) -> Option<&'a str> {
    let word = text.split_whitespace().next()?;
    let word = word.trim_matches('`');
    let word = word.trim_end_matches([':', ',', ';', '.', ')', '`']);
    // A file has an extension, a directory a trailing `/`; a bare word is a path
    // only when it names a directory that holds scanned files.
    let last = word.rsplit('/').next().unwrap_or("");
    let shaped = word.ends_with('/') || last.contains('.') || dirs.contains(word);
    (shaped && ROOTS.iter().any(|r| word.starts_with(r))).then_some(word)
}

/// A header naming a path that is not the file's own; `dirs` is [`dirs_of`] the
/// scanned files.
pub fn check(path: &str, source: &str, dirs: &BTreeSet<String>) -> Vec<Finding> {
    let wgsl = path.ends_with(".wgsl");
    let mark = if wgsl { "//" } else { "//!" };
    let Some((i, text)) = source
        .lines()
        .take(HEAD)
        .enumerate()
        .find_map(|(i, l)| l.trim_start().strip_prefix(mark).map(|t| (i, t)))
    else {
        return Vec::new();
    };
    // A shader's header is its first line; a `//` further down is just a comment.
    if wgsl && i != 0 {
        return Vec::new();
    }
    let Some(named) = leading_path(text, dirs) else {
        return Vec::new();
    };
    let dir = path
        .strip_suffix("mod.rs")
        .filter(|_| path.ends_with("/mod.rs"));
    let ok = named == path || dir.is_some_and(|d| named == d || named == d.trim_end_matches('/'));
    if ok {
        return Vec::new();
    }
    let expected = match dir {
        Some(d) => format!("`{path}` or `{d}`"),
        None => format!("`{path}`"),
    };
    vec![Finding {
        path: path.to_string(),
        line: i + 1,
        rule: RULE,
        message: format!("the header names `{named}`; this file is {expected}"),
    }]
}

/// The rule over the shaders under `engine/`, at their paths.
fn shaders(paths: &[String], dirs: &BTreeSet<String>) -> Vec<Finding> {
    paths
        .iter()
        .flat_map(|path| {
            let source = fs::read_to_string(path).unwrap_or_default();
            let mut found = check(path, &source, dirs);
            found.retain(|f| !finding::allowed(&source, f.line, f.rule));
            found.extend(finding::unexplained(path, &source));
            found
        })
        .collect()
}

/// The directories a bare header word may name: those of the scanned `.rs` files
/// and of the scanned shaders, so a bare `engine/shaders/postfx` counts too.
pub fn scanned_dirs(files: &[String], shaders: &[String]) -> BTreeSet<String> {
    dirs_of(files.iter().chain(shaders))
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    let wgsl: Vec<String> = crate::size::shaders_in(Path::new("engine"))
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let dirs = scanned_dirs(&scan.files, &wgsl);
    let mut found = scan.each_file(|path, source| check(path, source, &dirs));
    found.extend(shaders(&wgsl, &dirs));
    found
}

#[cfg(test)]
#[path = "self_path_tests.rs"]
mod tests;
