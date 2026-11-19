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
//!   `OnceLock`'s `.set(` or `.get_or_init(` (a cache filled), an `RwLock`'s
//!   `.write()`, or a `.lock()` whose guard (past one `.unwrap()`,
//!   `.unwrap_or_else(..)` or `.expect(..)`) is written through: assigned
//!   (`*X.lock().unwrap() = …`), a field or index of it assigned (`.count += 1`),
//!   or a mutating first call (`.push*(`, `.pop*(`, `.insert(`, `.sort*(`, …, a
//!   one-argument `.replace(`). A chain rustfmt split onto `.`-led lines is read
//!   as one line;
//! - a **test hook** writing one: the same write on a production line inside a
//!   `fn` whose doc says it is a test hook, or anywhere in a file whose `//!`
//!   header says so (`crates/CLAUDE.md`, *Test hooks*), which every test that
//!   arms it inherits.
//!
//! The sound shapes are state owned by the value under test
//! (`lighting::lightmap::Rays::counted`), a `thread_local!` when the code under
//! test runs on the test's own thread, and a child process when the state is truly
//! process-wide. A write that is sound anyway (a one-shot, idempotent install)
//! says why with `// lint: allow(test-static-write) <why>`.
//!
//! A line scan, not a type checker. It cannot see a guard bound on one line and
//! written through on the next (`let mut g = X.lock()…; g.push(…)`), a guard
//! call whose argument spans lines, a `LazyLock` filled by being read, a static
//! reached through a function that is not marked as a test hook, process-wide
//! state behind another crate's API (a registry a call appends to), nor
//! `crates/test-support/`, which holds process-wide statics (`logs.rs`' `KEPT`,
//! `ONCE`) that the shared test-line definition does not count as test code: those stay the nightly one-process run's to catch.
use super::finding::Finding;
use super::scan::Scan;
use crate::code::bare_lines;
use std::collections::{BTreeMap, BTreeSet};

mod writes;
pub use writes::writes;

pub const RULE: &str = "test-static-write";

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

/// Does the file's `//!` header call the whole module a test hook?
fn hook_module(raw: &[&str]) -> bool {
    raw.iter()
        .take_while(|l| l.trim_start().starts_with("//!"))
        .any(|l| l.to_lowercase().contains("test hook"))
}

/// Line `i` of `bare`, with the `.`-led lines rustfmt split its chain onto.
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

/// The findings of one file (`bare`, its [`bare_lines`]), given which `static`
/// names are process-wide for it.
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

/// The directory `path` sits in, `""` at the root.
fn dir(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(d, _)| d)
}

/// The rule over every scanned file. A name a file declares is what that file
/// says; any other is process-wide when some file declares it so, unless a file
/// in its directory or the one above declares it thread-local: the same name is
/// a `static` in one module and a `thread_local!` in another.
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
