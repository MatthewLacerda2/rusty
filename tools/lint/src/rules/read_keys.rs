//! `read-keys`: only the binaries an author spends from opt in to provider keys
//! (#974), and each of them still does.
//!
//! `dev::providers::read_keys` lets a process read `*_API_KEY` and the `.env`; a
//! process that never calls it holds no key, which is what keeps every test from
//! spending. So a call anywhere but one of [`MUST_CALL`] is a finding: a library or
//! test calling it would hand keys to every test in its process, and another binary
//! (`play`, `game`, a new one) would read keys nobody spends from. And each of
//! [`MUST_CALL`] missing its call is a finding too, since that binary would silently
//! lose every provider verb. Only code counts: a call in a comment or a string
//! literal is none. `tools/` is not scanned. A deliberate exception takes
//! `// lint: allow(read-keys) <why>`.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::bare_lines;

pub const RULE: &str = "read-keys";

/// The binaries that read provider keys, by root file and `[[bin]]` name: the
/// editor (with `dev`), `session` and `session-mcp`. `docs/providers.md` names the
/// same three, and [`KEY_READERS`]'s list must name them in this order.
pub const MUST_CALL: &[(&str, &str)] = &[
    ("src/main.rs", "rusty"),
    ("src/bin/session.rs", "session"),
    ("src/bin/session_mcp.rs", "session-mcp"),
];

/// Where the engine keeps the same list, for the refusal that names them.
pub const KEY_READERS: &str = "crates/dev/src/providers/credentials.rs";

/// Is `path` one of [`MUST_CALL`]'s roots?
fn spends(path: &str) -> bool {
    MUST_CALL.iter().any(|(root, _)| *root == path)
}

/// The lines of `source` that call `read_keys` in code (its definition, a comment
/// and a string literal are not calls).
fn calls(source: &str) -> Vec<usize> {
    bare_lines(source)
        .iter()
        .enumerate()
        .filter(|(_, code)| code.contains("read_keys(") && !code.contains("fn read_keys("))
        .map(|(i, _)| i + 1)
        .collect()
}

/// Every call outside [`MUST_CALL`].
pub fn check(path: &str, source: &str) -> Vec<Finding> {
    if spends(path) || path.starts_with("tools/") {
        return Vec::new();
    }
    calls(source)
        .into_iter()
        .map(|line| Finding {
            path: path.to_string(),
            line,
            rule: RULE,
            message: "only the editor, `session` and `session-mcp` call `read_keys` \
                      (docs/providers.md): anywhere else it hands provider keys to a process \
                      nobody spends from, or to every test in it. A binary that truly spends \
                      joins `MUST_CALL` in tools/lint/src/rules/read_keys.rs, and \
                      `KEY_READERS` in crates/dev/src/providers/credentials.rs"
                .to_string(),
        })
        .collect()
}

/// A finding when `path`, one of [`MUST_CALL`], no longer opts in.
pub fn missing(path: &str, source: &str) -> Option<Finding> {
    calls(source).is_empty().then(|| Finding {
        path: path.to_string(),
        line: 1,
        rule: RULE,
        message: "this binary must call `rusty::dev::providers::read_keys()` first thing, \
                  or every provider verb in it is refused for want of a key"
            .to_string(),
    })
}

/// A finding when [`KEY_READERS`]'s `pub const KEY_READERS` line does not list
/// exactly [`MUST_CALL`]'s binary names, in order.
pub fn readers_drifted(source: &str) -> Option<Finding> {
    let want: Vec<&str> = MUST_CALL.iter().map(|(_, bin)| *bin).collect();
    let (line, names) = source
        .lines()
        .enumerate()
        .find(|(_, l)| l.trim_start().starts_with("pub const KEY_READERS"))
        .map(|(i, l)| (i + 1, l.split('"').skip(1).step_by(2).collect::<Vec<_>>()))
        .unwrap_or((1, Vec::new()));
    (names != want).then(|| Finding {
        path: KEY_READERS.to_string(),
        line,
        rule: RULE,
        message: format!(
            "`KEY_READERS` must list the binaries `read-keys` lets read keys, on one line: \
             {want:?} (tools/lint/src/rules/read_keys.rs `MUST_CALL`); it lists {names:?}"
        ),
    })
}

/// The rule over every scanned file, each of [`MUST_CALL`], and [`KEY_READERS`].
pub fn run(scan: &Scan) -> Vec<Finding> {
    let mut found = scan.each_file(check);
    found.extend(
        MUST_CALL
            .iter()
            .filter_map(|(root, _)| missing(root, scan.source(root))),
    );
    found.extend(readers_drifted(scan.source(KEY_READERS)));
    found
}

#[cfg(test)]
#[path = "read_keys_tests.rs"]
mod tests;
