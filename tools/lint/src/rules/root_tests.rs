//! `root-tests`: the root package builds no unit-test harness, so its sources
//! hold no tests.
//!
//! The root package is the binaries and the integration suite; its lib only
//! re-exports the member crates. A test harness for its lib or a binary is one
//! more rustc run and link against the whole engine on every `cargo test` and
//! `nextest run`, running no test. So `Cargo.toml` sets `test = false` on `[lib]`
//! and on every `[[bin]]`, and `autobins = false` on `[package]`, so a binary
//! cargo would find under `src/bin/` on its own (with a harness) is declared
//! instead: undeclared, it is not compiled, and `orphan-module` names it. The
//! findings: a root target or `[package]` missing its key (the harness comes
//! back), and a test gate in root `src/` (a test no harness builds). Tests go in
//! `tests/` or in a member crate. Nothing excuses either (`Allow::Never`): the fix
//! is the manifest line or a moved test.

use super::finding::Finding;
use super::scan::Scan;

pub const RULE: &str = "root-tests";

/// The root manifest's path, where its targets are read.
const MANIFEST: &str = "Cargo.toml";

/// A manifest line with its comment and every space or tab dropped:
/// `[[bin]]  # x` reads `[[bin]]`, `test\t= false` reads `test=false`.
fn bare(line: &str) -> String {
    let code = line.split('#').next().unwrap_or("");
    code.chars().filter(|c| !c.is_whitespace()).collect()
}

/// What each section the rule holds must say, and why, by its header.
fn needs(header: &str) -> Option<(&'static str, &'static str)> {
    match header {
        "[package]" => Some((
            "autobins=false",
            "add `autobins = false` here: cargo would build a binary it finds under \
             `src/bin/` with a test harness; declare each in a `[[bin]]` instead",
        )),
        "[lib]" | "[[bin]]" => Some((
            "test=false",
            "a root target builds no test harness: add `test = false` here (each \
             harness is one more rustc run and link against the whole engine on every \
             test build, and the root's tests live in `tests/`)",
        )),
        _ => None,
    }
}

/// A finding at the header of every `[package]` without `autobins = false` and
/// every `[lib]` or `[[bin]]` without `test = false`.
pub fn check_manifest(text: &str) -> Vec<Finding> {
    // Each held section: its header line, the key it needs, its message, and
    // whether it said the key.
    let mut sections: Vec<(usize, &str, &str, bool)> = Vec::new();
    let mut held = false;
    for (i, line) in text.lines().enumerate() {
        let line = bare(line);
        if line.starts_with('[') {
            let need = needs(&line);
            held = need.is_some();
            if let Some((key, message)) = need {
                sections.push((i + 1, key, message, false));
            }
        } else if let Some((_, key, _, said)) = sections.last_mut().filter(|_| held) {
            *said |= line == *key;
        }
    }
    sections
        .into_iter()
        .filter(|(.., said)| !said)
        .map(|(line, _, message, _)| Finding {
            path: MANIFEST.to_string(),
            line,
            rule: RULE,
            message: message.to_string(),
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
