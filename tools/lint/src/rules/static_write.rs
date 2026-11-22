//! says why with `// lint: allow(test-static-write) <why>`.
use super::finding::Finding;
use super::scan::Scan;
use crate::code::bare_lines;
use std::collections::{BTreeMap, BTreeSet};

mod writes;
pub use writes::writes;

pub const RULE: &str = "test-static-write";

#[derive(Default)]
pub struct Decls {
    pub process: BTreeSet<String>,
    pub local: BTreeSet<String>,
}

pub fn decls(bare: &[String]) -> Decls {
    let mut out = Decls::default();
    let mut depth = 0i32;
    let mut local_at: Option<i32> = None;
    for line in bare {
        if local_at.is_none() && line.contains("thread_local!") {
            local_at = Some(depth);
        }
        if let Some(name) = declared(line) {
            if local_at.is_some() {
                out.local.insert(name);
            } else {
                out.process.insert(name);
            }
        }
        for c in line.chars() {
            depth += match c {
                '{' | '(' | '[' => 1,
                '}' | ')' | ']' => -1,
                _ => 0,
            };
        }
        if local_at.is_some_and(|at| depth <= at) {
            local_at = None;
        }
    }
    out
}

fn declared(line: &str) -> Option<String> {
    let mut words = line
        .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '\''))
        .filter(|w| !w.is_empty());
    words.find(|w| *w == "static")?;
    let name = match words.next()? {
        "mut" => words.next()?,
        w => w,
    };
    let rest = &line[line.find(name)? + name.len()..];
    let upper = name
        .chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    (upper && rest.trim_start().starts_with(':')).then(|| name.to_string())
}

pub fn in_test_hook(bare: &[String], raw: &[&str], at: usize) -> bool {
    let indent = |l: &str| l.len() - l.trim_start().len();
    let mine = indent(&bare[at]);
    let Some(head) = (0..at).rev().find(|&i| {
        let l = &bare[i];
        indent(l) < mine && l.split(|c: char| !c.is_alphanumeric()).any(|w| w == "fn")
    }) else {
        return false;
    };
    raw[..head]
        .iter()
        .rev()
        .map(|l| l.trim_start())
        .take_while(|l| l.starts_with("//") || l.starts_with("#["))
        .any(|l| l.to_lowercase().contains("test hook"))
}

fn hook_module(raw: &[&str]) -> bool {
    raw.iter()
        .take_while(|l| l.trim_start().starts_with("//!"))
        .any(|l| l.to_lowercase().contains("test hook"))
}

fn chained(bare: &[String], i: usize) -> std::borrow::Cow<'_, str> {
    let next = bare[i + 1..]
        .iter()
        .take_while(|l| l.trim_start().starts_with('.'));
    let mut lines = next.peekable();
    if lines.peek().is_none() {
        return bare[i].as_str().into();
    }
    let mut joined = bare[i].trim_end().to_string();
    lines.for_each(|l| joined.push_str(l.trim()));
    joined.into()
}

pub fn check_file(
    path: &str,
    source: &str,
    bare: &[String],
    test: &[bool],
    is_process: &dyn Fn(&str) -> bool,
) -> Vec<Finding> {
    let at = crate::modules::module_path(path);
    if !(at.starts_with("src/") || at.starts_with("tests/")) {
        return Vec::new();
    }
    let raw: Vec<&str> = source.lines().collect();
    let module = hook_module(&raw);
    let hooks = module || source.to_lowercase().contains("test hook");
    (0..bare.len())
        .filter(|&i| test.get(i).copied().unwrap_or(false) || hooks)
        .filter(|&i| writes(&chained(bare, i), is_process))
        .filter_map(|i| {
            let message = if test.get(i).copied().unwrap_or(false) {
                "a test writes a process-wide `static`: in the one-process run (`make \
                 test-one-process`) every test of the binary shares it"
            } else if module || in_test_hook(bare, &raw, i) {
                "a test hook writes a process-wide `static`: every test that arms it \
                 shares it in the one-process run (`make test-one-process`)"
            } else {
                return None;
            };
            Some(Finding {
                path: path.to_string(),
                line: i + 1,
                rule: RULE,
                message: format!(
                    "{message}; keep the state on the value under test, in a \
                     `thread_local!` (the code runs on the test's thread), or in a child \
                     process, or allow it inline saying why it is sound \
                     (docs/linting.md, `test-static-write`)"
                ),
            })
        })
        .collect()
}

fn dir(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(d, _)| d)
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    let bare: BTreeMap<&str, Vec<String>> = scan
        .files
        .iter()
        .map(|p| (p.as_str(), bare_lines(scan.source(p))))
        .collect();
    let all: BTreeMap<&str, Decls> = bare.iter().map(|(p, b)| (*p, decls(b))).collect();
    let process: BTreeSet<&str> = all
        .values()
        .flat_map(|d| &d.process)
        .map(String::as_str)
        .collect();
    let mut local: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (p, d) in &all {
        local
            .entry(dir(p))
            .or_default()
            .extend(d.local.iter().map(String::as_str));
    }
    let none = BTreeSet::new();
    scan.each_file(|p, s| {
        let own = &all[p];
        let near = [
            local.get(dir(p)).unwrap_or(&none),
            local.get(dir(dir(p))).unwrap_or(&none),
        ];
        let is_process = |n: &str| {
            own.process.contains(n)
                || (!own.local.contains(n)
                    && process.contains(n)
                    && !near.iter().any(|l| l.contains(n)))
        };
        check_file(p, s, &bare[p], scan.test_lines(p), &is_process)
    })
}

#[cfg(test)]
#[path = "static_write_tests.rs"]
mod tests;
