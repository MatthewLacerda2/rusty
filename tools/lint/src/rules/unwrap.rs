//! `unwrap-deny`: every sim module denies bare `.unwrap()` (#195, #962).
//!
//! A panic mid-frame kills an unattended play-test, so the sim's production code
//! never unwraps. Clippy does the banning; this rule keeps the ban's scope equal to
//! the layer table's sim rows (`layers::sim_dirs`), so a new sim module is covered
//! the day it gets its row:
//! - each sim row's root `src/<module>/mod.rs` carries [`DENY`];
//! - no file under a sim module lowers that level again (`allow`, `expect`, `warn`)
//!   for `clippy::unwrap_used` or the `clippy::restriction` group that holds it.
//!
//! Platform modules are out of scope. `.expect("why")` stays the sanctioned escape.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "unwrap-deny";
/// The attribute every sim root carries.
pub const DENY: &str = "#![deny(clippy::unwrap_used)]";
/// Names that turn the lint off: the lint, and the one group that holds it.
const LINTS: [&str; 2] = ["clippy::unwrap_used", "clippy::restriction"];

/// The findings for one scanned file, given the sim rows' directories (`src/<m>`).
pub fn check(path: &str, source: &str, sim_dirs: &[String]) -> Vec<Finding> {
    let Some(dir) = sim_dirs.iter().find(|d| path.starts_with(&format!("{d}/"))) else {
        return Vec::new();
    };
    let finding = |line, message: String| Finding {
        path: path.to_string(),
        line,
        rule: RULE,
        message,
    };
    let stripped = code_lines(source);
    let code: Vec<(usize, &str)> = stripped
        .iter()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .collect();
    let mut out = Vec::new();
    if path == format!("{dir}/mod.rs") && !code.iter().any(|(_, l)| *l == DENY) {
        out.push(finding(
            1,
            format!("a sim module's root must carry `{DENY}`"),
        ));
    }
    for (line, l) in code {
        let lowers = !(l.contains("deny(") || l.contains("forbid("));
        if lowers && LINTS.iter().any(|n| names_lint(l, n)) {
            out.push(finding(
                line,
                "a sim module may not lift the unwrap ban: use `?`, a handled `None`, \
                 or `.expect(\"why\")`"
                    .into(),
            ));
        }
    }
    out
}

/// The sim rows whose root `src/<module>/mod.rs` is not among `files`.
pub fn missing_roots(files: &[String], sim_dirs: &[String]) -> Vec<Finding> {
    sim_dirs
        .iter()
        .map(|d| format!("{d}/mod.rs"))
        .filter(|root| !files.contains(root))
        .map(|root| Finding {
            path: root,
            line: 1,
            rule: RULE,
            message: format!("a sim row needs a `mod.rs` root carrying `{DENY}`"),
        })
        .collect()
}

/// Whether `code` names the lint `name` as a whole path.
fn names_lint(code: &str, name: &str) -> bool {
    code.match_indices(name).any(|(at, _)| {
        let next = code[at + name.len()..].chars().next();
        !next.is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}

/// The rule over the layer table's sim rows and every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    let mut found = missing_roots(&scan.files, &scan.sim_dirs);
    found.extend(scan.each_file(|p, s| check(p, s, &scan.sim_dirs)));
    found
}

#[cfg(test)]
#[path = "unwrap_tests.rs"]
mod tests;
