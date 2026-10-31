//! `self-path`: a header that opens with a path names its own file (#1654).
//!
//! `//! src/scene/prefab/link.rs — …` is a promise about where the file is. The
//! directory regroupings `dir-size` drives (#1367) move files and leave the header
//! behind, and a stale path sends an agent to a place that is not there (the #1650
//! audit fixed about 35 by hand). When a source file's first module-doc line
//! (`//!`) or a shader's first line (`// …`) opens with a path under a repository
//! root (`src/`, `tests/`, `tools/`, `fuzz/`, `engine/`, `.github/`), that path
//! must be the file's own. A `mod.rs` may instead name its directory, `src/a/b/`
//! or `src/a/b/mod.rs` (a directory is written with its trailing `/`, a file with
//! its extension; `tools/lint` is prose). A header that opens with no path, or a file with no
//! header, is left alone: the rule never asks for one. A header that names another
//! file on purpose says why with `// lint: allow(self-path) <why>` on the line.
//!
//! The rule reads `engine/**/*.wgsl` itself, as `fullscreen-triangle` does, since
//! `Scan` holds only Rust.

use super::finding::{self, Finding};
use super::scan::Scan;
use std::fs;
use std::path::Path;

pub const RULE: &str = "self-path";

/// What a header's leading path starts with to count as a path.
const ROOTS: &[&str] = &["src/", "tests/", "tools/", "fuzz/", "engine/", ".github/"];

/// How many leading lines are searched for the header (attributes may come first).
const HEAD: usize = 10;

/// The header's leading path, if it opens with one: the first word, without
/// backticks or trailing punctuation.
fn leading_path(text: &str) -> Option<&str> {
    let word = text.split_whitespace().next()?;
    let word = word.trim_matches('`');
    let word = word.trim_end_matches([':', ',', ';', '.', ')', '`']);
    // A file has an extension, a directory a trailing `/`: `tools/lint` is prose.
    let last = word.rsplit('/').next().unwrap_or("");
    let shaped = word.ends_with('/') || last.contains('.');
    (shaped && ROOTS.iter().any(|r| word.starts_with(r))).then_some(word)
}

/// A header naming a path that is not the file's own.
pub fn check(path: &str, source: &str) -> Vec<Finding> {
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
    let Some(named) = leading_path(text) else {
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

/// The rule over every shader under `engine/`.
fn shaders() -> Vec<Finding> {
    crate::size::shaders_in(Path::new("engine"))
        .iter()
        .flat_map(|p| {
            let path = p.to_string_lossy().replace('\\', "/");
            let source = fs::read_to_string(p).unwrap_or_default();
            let mut found = check(&path, &source);
            found.retain(|f| !finding::allowed(&source, f.line, f.rule));
            found.extend(finding::unexplained(&path, &source));
            found
        })
        .collect()
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    let mut found = scan.each_file(check);
    found.extend(shaders());
    found
}

#[cfg(test)]
#[path = "self_path_tests.rs"]
mod tests;
