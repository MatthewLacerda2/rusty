//! `test-static-write`: a test never writes a process-wide `static`.
//!
//! nextest gives each test a process of its own, but the one-process run
//! (`cargo test`, `make test-one-process`) shares one between every test of a
//! binary, on many threads (`docs/testing.md`, *Every module can share one
//! process*). A test that stores to a crate `static` (a flag it flips, a counter it
//! resets, a cache it fills) passes the gates and then fails, or silently stops
//! testing, there. Two shapes are findings:
//!
//! - a **test line** writing a `static` declared outside a `thread_local!`: an
//!   atomic's `.store(`, `.swap(`, `.fetch_*(` or `.compare_exchange*(`, a
//!   `OnceLock`'s `.set(`, an `RwLock`'s `.write()`, or a `.lock()` that mutates on
//!   the same line (`*X.lock().unwrap() = …`, `X.lock().unwrap().push(…)`);
//! - a **test hook** writing one: the same write on a production line inside a
//!   `fn` whose doc says it is a test hook (`crates/CLAUDE.md`, *Test hooks*), which
//!   every test that arms it inherits.
//!
//! The sound shapes are state owned by the value under test
//! (`lighting::lightmap::Rays::counted`), a `thread_local!` when the code under
//! test runs on the test's own thread, and a child process when the state is truly
//! process-wide. A write that is sound anyway (a one-shot, idempotent install)
//! says why with `// lint: allow(test-static-write) <why>`.
//!
//! A line scan, not a type checker. It cannot see a guard bound on one line and
//! written through on the next, a static reached through a function that is not
//! marked as a test hook, nor process-wide state behind another crate's API (a
//! registry a call appends to): those stay the nightly one-process run's to catch.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::bare_lines;
use std::collections::BTreeSet;

pub const RULE: &str = "test-static-write";

/// The methods that write an atomic or a `OnceLock` in place.
const WRITES: &[&str] = &[".store(", ".swap(", ".fetch_", ".compare_exchange", ".set("];

/// What a guard from `.lock()` is written through on the same line.
const MUTATES: &[&str] = &[
    ".push(",
    ".insert(",
    ".clear(",
    ".remove(",
    ".retain(",
    ".extend(",
    ".take(",
    ".replace(",
    ".truncate(",
    ".drain(",
    ".entry(",
    ".get_mut(",
];

/// The `static` names one file declares: process-wide, and inside a `thread_local!`.
#[derive(Default)]
pub struct Decls {
    pub process: BTreeSet<String>,
    pub local: BTreeSet<String>,
}

/// The statics `bare` (a file's [`bare_lines`]) declares.
pub fn decls(bare: &[String]) -> Decls {
    let mut out = Decls::default();
    let mut depth = 0i32;
    // The depth a `thread_local!` opened at, while its body is still open.
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

/// The name a `static NAME:` on `line` declares, `static mut` included.
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

/// Does `line` write one of `names`, by [`WRITES`], `.write()`, or a `.lock()` it
/// mutates through? The name must stand alone (a path's last segment counts).
pub fn writes(line: &str, names: &BTreeSet<String>) -> bool {
    names.iter().any(|name| {
        line.match_indices(name.as_str()).any(|(at, _)| {
            let before = line[..at].chars().next_back();
            if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                return false;
            }
            let after = &line[at + name.len()..];
            if WRITES.iter().any(|w| after.starts_with(w)) || after.starts_with(".write()") {
                return true;
            }
            after.starts_with(".lock()")
                && (MUTATES.iter().any(|m| after.contains(m))
                    || (line[..at].trim_start().starts_with('*') && assigns(after)))
        })
    })
}

/// Does `rest` assign (` = `, ` += `, …), not compare?
fn assigns(rest: &str) -> bool {
    rest.match_indices('=').any(|(i, _)| {
        let prev = rest[..i].chars().next_back();
        let next = rest[i + 1..].chars().next();
        next != Some('=') && !matches!(prev, Some('=' | '!' | '<' | '>'))
    })
}

/// Is the production line `at` of `bare` inside a `fn` whose doc (`raw`'s comment
/// and attribute lines right above it) calls it a test hook?
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

/// The findings of one file, given the process-wide statics it can name.
pub fn check_file(
    path: &str,
    source: &str,
    test: &[bool],
    names: &BTreeSet<String>,
) -> Vec<Finding> {
    let at = crate::modules::module_path(path);
    if !(at.starts_with("src/") || at.starts_with("tests/")) {
        return Vec::new();
    }
    let bare = bare_lines(source);
    let raw: Vec<&str> = source.lines().collect();
    (0..bare.len())
        .filter(|&i| writes(&bare[i], names))
        .filter_map(|i| {
            let message = if test.get(i).copied().unwrap_or(false) {
                "a test writes a process-wide `static`: in the one-process run (`make \
                 test-one-process`) every test of the binary shares it"
            } else if in_test_hook(&bare, &raw, i) {
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

/// The rule over every scanned file. A name counts as process-wide unless the
/// file, or a file in its directory or the one above, declares it thread-local:
/// the same name is a `static` in one module and a `thread_local!` in another.
pub fn run(scan: &Scan) -> Vec<Finding> {
    let all: Vec<(&str, Decls)> = scan
        .files
        .iter()
        .map(|p| (p.as_str(), decls(&bare_lines(scan.source(p)))))
        .collect();
    let process: BTreeSet<String> = all.iter().flat_map(|(_, d)| d.process.clone()).collect();
    let dir = |p: &str| p.rsplit_once('/').map_or("", |(d, _)| d).to_string();
    scan.each_file(|p, s| {
        let near = [dir(p), dir(&dir(p))];
        let local: BTreeSet<&String> = all
            .iter()
            .filter(|(q, _)| near.contains(&dir(q)))
            .flat_map(|(_, d)| &d.local)
            .collect();
        let names = process
            .iter()
            .filter(|n| !local.contains(n))
            .cloned()
            .collect();
        check_file(p, s, scan.test_lines(p), &names)
    })
}

#[cfg(test)]
#[path = "static_write_tests.rs"]
mod tests;
