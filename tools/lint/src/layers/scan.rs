use super::extent::item_end;
use super::gate::{is_test_gate, test_only_files};
use crate::code::{strip_comment, Open};
use crate::modules;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub struct Tree {
    pub root: PathBuf,
    sources: BTreeMap<PathBuf, String>,
    test_only: BTreeSet<PathBuf>,
}

impl Tree {
    pub fn read(root: &Path) -> Self {
        let mut files = Vec::new();
        for m in modules::all(root) {
            files.extend(walk(&modules::dir(root, &m)));
        }
        let sources: BTreeMap<PathBuf, String> = files
            .iter()
            .map(|f| (f.clone(), fs::read_to_string(f).unwrap_or_default()))
            .collect();
        let test_only = test_only_files(&files, |f| sources.get(f).map(String::as_str));
        Self {
            root: root.to_path_buf(),
            sources,
            test_only,
        }
    }

    pub fn owner(&self, file: &Path) -> Option<String> {
        let rel = file.strip_prefix(&self.root).unwrap_or(file);
        modules::crate_module(&rel.to_string_lossy()).map(str::to_string)
    }

    pub fn is_test_only(&self, path: &Path) -> bool {
        self.test_only.contains(path)
    }

    pub fn live_files(&self) -> impl Iterator<Item = (&Path, &str)> {
        self.sources
            .iter()
            .filter(|(f, _)| !self.is_test_only(f))
            .map(|(f, text)| (f.as_path(), text.as_str()))
    }
}

pub struct Ref {
    pub line: usize,
    pub module: String,
}

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

pub use crate::code::is_ident;

pub fn refs(content: &str) -> Vec<Ref> {
    let mut out = Vec::new();
    for (line, code) in live_lines(content) {
        let named = sibling_refs(&code);
        out.extend(named.into_iter().map(|module| Ref { line, module }));
    }
    out
}

fn sibling_refs(code: &str) -> Vec<String> {
    code.match_indices(modules::CRATE_PREFIX)
        .filter(|(at, _)| !code[..*at].chars().next_back().is_some_and(is_ident))
        .filter_map(|(at, p)| {
            let rest = &code[at + p.len()..];
            let end = rest.find(|c: char| !is_ident(c)).unwrap_or(rest.len());
            (end > 0 && rest[end..].starts_with("::")).then(|| rest[..end].to_string())
        })
        .collect()
}

pub fn live_lines(content: &str) -> Vec<(usize, Cow<'_, str>)> {
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

#[cfg(test)]
#[path = "scan_tests.rs"]
mod tests;
