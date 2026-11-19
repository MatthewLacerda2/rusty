//! `one-logger`: one `log` logger a test binary. A package whose manifest names
//! `rusty-logging` reads its tests' warnings through it
//! (`rusty_logging::init_for_tests` and `rusty_logging::forward`), never through
//! `rusty_test_support::logs`: `log::set_logger` takes the first logger a process
//! installs, so under plain `cargo test` the two would race and whichever came
//! second would capture nothing. nextest runs a test a process, so the gate never
//! sees the race; this rule does, and each `init` also panics when it lost.
//! `rusty_test_support::logs` stays for the crates below `rusty-logging`.
//!
//! It reads whole `use` items (a list rustfmt splits over lines, an alias of the
//! crate, a glob of it or of `logs`) and every path naming `logs` through the
//! crate or an alias. It cannot see a path a macro builds, nor `logs` re-exported
//! by another crate.

use super::finding::Finding;
use super::scan::{names, Scan};
use crate::code::{bare_lines, is_ident};
use crate::layers::flatten_use;

pub const RULE: &str = "one-logger";

/// The crate whose capture a package above `rusty-logging` never names.
const CRATE: &str = "rusty_test_support";

/// Stands for a glob in a flattened `use` path ([`flatten_use`] drops globs): an
/// identifier no real path holds.
const GLOB: &str = "__glob__";

/// Findings on the lines of `source` (at `path`) that name test-support's capture,
/// when its package's `manifest` text names `rusty-logging`.
pub fn check(path: &str, manifest: &str, source: &str) -> Vec<Finding> {
    match names(manifest, "rusty-logging") {
        true => found(path, source),
        false => Vec::new(),
    }
}

/// The findings of [`check`] in a package that names `rusty-logging`.
fn found(path: &str, source: &str) -> Vec<Finding> {
    let code = bare_lines(source).join("\n");
    let line = |at: usize| code[..at].matches('\n').count() + 1;
    let items = uses(&code);
    let mut roots = vec![CRATE.to_string()];
    for (_, tree) in &items {
        let paths = flatten_use(tree);
        roots.extend(
            paths
                .into_iter()
                .filter(|(p, _)| p == &[CRATE])
                .map(|(_, b)| b),
        );
    }
    let mut lines: Vec<usize> = items
        .iter()
        .filter(|(_, tree)| {
            flatten_use(&tree.replace('*', GLOB))
                .iter()
                .any(|(p, _)| brings_logs(p, &roots))
        })
        .map(|&(at, _)| line(at))
        .collect();
    for root in &roots {
        lines.extend(paths_to(&code, &format!("{root}::logs")).map(line));
    }
    lines.sort_unstable();
    lines.dedup();
    lines.into_iter().map(|line| finding(path, line)).collect()
}

/// Whether a flattened `use` path brings in `logs` through one of `roots`.
fn brings_logs(path: &[String], roots: &[String]) -> bool {
    matches!(path, [root, next, ..] if roots.contains(root) && (next == "logs" || next == GLOB))
}

/// Each `use` item of `code`: its offset and its tree, up to its `;`.
fn uses(code: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for at in paths_to(code, "use") {
        let rest = &code[at + 3..];
        if rest.starts_with(char::is_whitespace) {
            let end = rest.find(';').unwrap_or(rest.len());
            out.push((at, rest[..end].trim().trim_start_matches("::").to_string()));
        }
    }
    out
}

/// The offsets in `code` where `path` starts as a whole path: no identifier
/// character on either side.
fn paths_to<'a>(code: &'a str, path: &'a str) -> impl Iterator<Item = usize> + 'a {
    code.match_indices(path)
        .map(|(at, _)| at)
        .filter(move |&at| {
            let before = code[..at].chars().next_back();
            let after = code[at + path.len()..].chars().next();
            !before.is_some_and(is_ident) && !after.is_some_and(is_ident)
        })
}

fn finding(path: &str, line: usize) -> Finding {
    Finding {
        path: path.to_string(),
        line,
        rule: RULE,
        message: "this package names `rusty-logging`, so its tests read warnings through \
                  `rusty_logging::init_for_tests` and `rusty_logging::forward`: \
                  `rusty_test_support::logs` installs a second logger, and under plain \
                  `cargo test` whichever comes second captures nothing"
            .to_string(),
    }
}

/// The rule over the root package and every member crate: each manifest against the
/// code under its `src/` and `tests/`.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.packages()
        .iter()
        .flat_map(|p| {
            p.files
                .iter()
                .flat_map(|f| check(f, &p.text, scan.source(f)))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(test)]
#[path = "one_logger_tests.rs"]
mod tests;
