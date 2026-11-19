//! `test-logger`: one `log` logger a test binary. A package whose manifest names
//! `rusty-logging` reads its tests' warnings through it
//! (`rusty_logging::init_for_tests` and `rusty_logging::forward`), never through
//! `rusty_test_support::logs`: `log::set_logger` takes the first logger a process
//! installs, so under plain `cargo test` the two would race and whichever came
//! second would capture nothing. nextest runs a test a process, so the gate never
//! sees the race; this rule does, and each `init` also panics when it lost.
//! `rusty_test_support::logs` stays for the crates below `rusty-logging`.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::bare_lines;

pub const RULE: &str = "test-logger";

/// Whether a line of code names test-support's capture (a path or a `use` list).
fn names_capture(code: &str) -> bool {
    code.contains("test_support::logs")
        || (code.contains("test_support::{") && code.contains("logs"))
}

/// Findings on the lines of `source` (at `path`) that name test-support's capture,
/// when its package's `manifest` text names `rusty-logging`.
pub fn check(path: &str, manifest: &str, source: &str) -> Vec<Finding> {
    let mut named = crate::layers::manifest_dependencies(manifest);
    named.extend(crate::layers::manifest_dev_dependencies(manifest));
    if !named.iter().any(|k| k == "rusty_logging") {
        return Vec::new();
    }
    bare_lines(source)
        .iter()
        .enumerate()
        .filter(|(_, code)| names_capture(code))
        .map(|(i, _)| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message: "this package names `rusty-logging`, so its tests read warnings through \
                      `rusty_logging::init_for_tests` and `rusty_logging::forward`: \
                      `rusty_test_support::logs` installs a second logger, and under plain \
                      `cargo test` whichever comes second captures nothing"
                .to_string(),
        })
        .collect()
}

/// The rule over the root package and every member crate: each manifest against the
/// code under its `src/` and `tests/`.
pub fn run(scan: &Scan) -> Vec<Finding> {
    let members = crate::modules::member_manifests(&scan.base);
    let members = members
        .iter()
        .map(|m| m.strip_prefix(&scan.base).unwrap_or(m))
        .map(|m| m.to_string_lossy().replace('\\', "/"));
    std::iter::once(String::from("Cargo.toml"))
        .chain(members)
        .flat_map(|manifest| {
            let root = manifest.trim_end_matches("Cargo.toml").to_string();
            let dirs = [format!("{root}src/"), format!("{root}tests/")];
            let text = std::fs::read_to_string(scan.base.join(&manifest)).unwrap_or_default();
            let inside = |f: &&String| dirs.iter().any(|d| f.starts_with(d.as_str()));
            scan.files
                .iter()
                .filter(inside)
                .flat_map(|f| check(f, &text, scan.source(f)))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(test)]
#[path = "test_logger_tests.rs"]
mod tests;
