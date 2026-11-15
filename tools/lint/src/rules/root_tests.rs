//! `root-tests`: the root package builds no unit-test harness, so its sources
//! hold no tests.
//!
//! The root package is the binaries and the integration suite; its lib only
//! re-exports the member crates. A test harness for its lib or a binary is one
//! more rustc run and link over the whole engine on every `cargo test` and
//! `nextest run` (nine of them, running no test), so `Cargo.toml` sets `test = false` on `[lib]` and on every
//! `[[bin]]`. Two findings hold that: a root target missing `test = false` (the
//! links come back), and a test gate in root `src/` (a test that would never run,
//! since no harness builds it). Tests go in `tests/` or in a member crate. Nothing
//! excuses either (`Allow::Never`): the fix is the manifest line or a moved test.

use super::finding::Finding;
use super::scan::Scan;

pub const RULE: &str = "root-tests";

/// The root manifest's path, where its targets are read.
const MANIFEST: &str = "Cargo.toml";

/// A finding at the header of every `[lib]` or `[[bin]]` section of `manifest`
/// that does not say `test = false`.
pub fn check_manifest(text: &str) -> Vec<Finding> {
    // Each target section's header line, and whether it said `test = false`.
    let mut sections: Vec<(usize, bool)> = Vec::new();
    let mut in_target = false;
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.starts_with('[') {
            in_target = line == "[lib]" || line == "[[bin]]";
            if in_target {
                sections.push((i + 1, false));
            }
        } else if let Some((_, said)) = sections.last_mut().filter(|_| in_target) {
            let bare = line.split('#').next().unwrap_or("").replace(' ', "");
            *said |= bare == "test=false";
        }
    }
    sections
        .into_iter()
        .filter(|(_, said)| !said)
        .map(|(line, _)| Finding {
            path: MANIFEST.to_string(),
            line,
            rule: RULE,
            message: "a root target builds no test harness: add `test = false` here \
                      (each harness is one more rustc run and link over the whole \
                      engine on every test build, and the root's tests live in `tests/`)"
                .to_string(),
        })
        .collect()
}

/// A finding at the first test line (`test`, per line) of a root `src/` file. The
/// raw path, not `modules::module_path`: that maps each member's `crates/<m>/src/`
/// to `src/<m>/` too, and only the root package's own files are meant.
pub fn check_source(path: &str, test: &[bool]) -> Option<Finding> {
    if !path.starts_with("src/") {
        return None;
    }
    let at = test.iter().position(|t| *t)?;
    Some(Finding {
        path: path.to_string(),
        line: at + 1,
        rule: RULE,
        message: "the root package builds no unit-test harness (`test = false` in \
                  Cargo.toml), so this test never runs: move it to `tests/` or into the \
                  member crate whose code it tests"
            .to_string(),
    })
}

/// The rule over the root manifest and every root `src/` file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    let text = std::fs::read_to_string(MANIFEST).unwrap_or_default();
    let mut found = check_manifest(&text);
    found.extend(
        scan.files
            .iter()
            .filter_map(|f| check_source(f, scan.test_lines(f))),
    );
    found
}

#[cfg(test)]
#[path = "root_tests_tests.rs"]
mod tests;
