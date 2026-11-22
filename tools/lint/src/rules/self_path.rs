//! file on purpose says why with `// lint: allow(self-path) <why>` on the line.

use super::finding::{self, Finding};
use super::scan::Scan;
use std::collections::BTreeSet;

pub const RULE: &str = "self-path";

const ROOTS: &[&str] = &[
    "src/", "crates/", "tests/", "tools/", "fuzz/", "engine/", ".github/",
];

const HEAD: usize = 10;

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

fn leading_path<'a>(text: &'a str, dirs: &BTreeSet<String>) -> Option<&'a str> {
    let word = text.split_whitespace().next()?;
    let word = word.trim_matches('`');
    let word = word.trim_end_matches([':', ',', ';', '.', ')', '`']);
    let last = word.rsplit('/').next().unwrap_or("");
    let shaped = word.ends_with('/') || last.contains('.') || dirs.contains(word);
    (shaped && ROOTS.iter().any(|r| word.starts_with(r))).then_some(word)
}

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

fn shaders(wgsl: &[(String, String)], dirs: &BTreeSet<String>) -> Vec<Finding> {
    wgsl.iter()
        .flat_map(|(path, source)| {
            let mut found = check(path, source, dirs);
            found.retain(|f| !finding::allowed(source, f.line, f.rule));
            found.extend(finding::unexplained(path, source));
            found
        })
        .collect()
}

pub fn scanned_dirs(files: &[String], shaders: &[String]) -> BTreeSet<String> {
    dirs_of(files.iter().chain(shaders))
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    let wgsl = scan.shaders();
    let paths: Vec<String> = wgsl.iter().map(|(p, _)| p.clone()).collect();
    let dirs = scanned_dirs(&scan.files, &paths);
    let mut found = scan.each_file(|path, source| check(path, source, &dirs));
    found.extend(shaders(wgsl, &dirs));
    found
}

#[cfg(test)]
#[path = "self_path_tests.rs"]
mod tests;
