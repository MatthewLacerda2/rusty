//! rustc's warnings, sorted into what the dead-pub signal reports (#1003).

use std::collections::BTreeSet;

/// One rustc warning, as `--message-format short` prints it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Warning {
    pub path: String,
    pub line: usize,
    pub message: String,
}

/// The warnings in `stderr` that name an unused item or import; and whether rustc
/// reported an error (then there is no report to trust).
pub fn parse(stderr: &str) -> (BTreeSet<Warning>, bool) {
    let mut found = BTreeSet::new();
    let mut failed = false;
    for line in stderr.lines() {
        let mut parts = line.splitn(4, ':');
        let (Some(path), Some(row), Some(_col), Some(rest)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let Ok(row) = row.parse() else { continue };
        let rest = rest.trim();
        failed |= rest.starts_with("error");
        if let Some(message) = rest
            .strip_prefix("warning: ")
            .filter(|m| is_dead(m) || is_import(m))
        {
            found.insert(Warning {
                path: path.to_string(),
                line: row,
                message: message.to_string(),
            });
        }
    }
    (found, failed)
}

fn is_dead(message: &str) -> bool {
    ["never used", "never read", "never constructed"]
        .iter()
        .any(|m| message.contains(m))
}

/// An unused import: in a `pub use`, the re-export of a type a public function
/// may still return, so it is listed apart (#940 kept them).
pub fn is_import(message: &str) -> bool {
    message.starts_with("unused import")
}

/// The report: dead even counting tests, used only by tests, and unused imports.
pub fn render(with_tests: &BTreeSet<Warning>, without_tests: &BTreeSet<Warning>) -> String {
    let dead = |w: &&Warning| !is_import(&w.message);
    let only_tests: BTreeSet<&Warning> = without_tests.difference(with_tests).collect();
    let mut out = String::from("dead-pub: a signal, never a gate (#1003)\n");
    let mut section = |title: &str, items: Vec<&Warning>| {
        out.push_str(&format!("\n{title} ({})\n", items.len()));
        for w in items {
            out.push_str(&format!("  {}:{}: {}\n", w.path, w.line, w.message));
        }
    };
    section(
        "Dead: nothing uses these, tests included",
        with_tests.iter().filter(dead).collect(),
    );
    section(
        "Only tests use these (fine for a test seam; otherwise dead)",
        only_tests.iter().copied().filter(dead).collect(),
    );
    section(
        "Unused imports (a re-exported type a public fn returns may keep its name)",
        with_tests
            .iter()
            .filter(|w| is_import(&w.message))
            .collect(),
    );
    out
}
