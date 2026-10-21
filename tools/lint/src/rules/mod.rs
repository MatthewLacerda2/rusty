//! The rules gate (#934): every check that reports a [`Finding`] and honours the
//! shared inline allow (`finding.rs`). A new rule is a module with its own
//! `<rule>_tests.rs`, plus one line in the sorted `mod` list and one row in the
//! sorted [`RULES`] (#911): not a new flag, gate or baseline.
//!
//! Usage: `cargo run --manifest-path tools/lint/Cargo.toml -- --rules`
//! Exit code 1 on any finding; report mirrored to `.lint/report.txt`.

mod finding;
mod scan;
mod test_code;

// sorted: one module per rule, each a row of `RULES`
mod atomic_write;
mod drag_range;
mod full_literal;
mod ignored;
mod log_tag;
mod markers;
mod prefix;
mod skip;
mod sorted;
mod subprocess;
mod temp_dir;
mod test_shape;
mod unwrap;

pub use finding::{allowed, Finding};
use scan::Scan;
use std::collections::BTreeMap;
use std::fs;
use std::process::exit;

const REPORT: &str = ".lint/report.txt";

/// Whether an inline `lint: allow(<rule>) <why>` may excuse a rule's findings.
#[derive(PartialEq)]
enum Allow {
    Inline,
    /// `ignored`: its fix is the `.gitignore` pattern, never an allow.
    Never,
}

/// A rule: every finding it has over the scan.
type Rule = fn(&Scan) -> Vec<Finding>;

/// Every rule. A new rule is one row, in its sorted place.
const RULES: &[(Rule, Allow)] = &[
    // sorted: rules are independent; allows apply to each rule's own findings
    (atomic_write::run, Allow::Inline),
    (drag_range::run, Allow::Inline),
    (full_literal::run, Allow::Inline),
    (ignored::run, Allow::Never),
    (log_tag::run, Allow::Inline),
    (markers::run, Allow::Inline),
    (prefix::run, Allow::Inline),
    (skip::run, Allow::Inline),
    (sorted::run, Allow::Inline),
    (subprocess::run, Allow::Inline),
    (temp_dir::run, Allow::Inline),
    (test_shape::run, Allow::Inline),
    (unwrap::run, Allow::Inline),
];

/// Entry point: run every rule over [`crate::size::SCAN_ROOTS`].
pub fn run() {
    let scan = Scan::load();
    let mut findings = Vec::new();
    for (rule, allow) in RULES {
        let mut found = rule(&scan);
        if *allow == Allow::Inline {
            found.retain(|f| !finding::allowed(scan.source(&f.path), f.line, f.rule));
        }
        findings.extend(found);
    }
    findings.extend(scan.each_file(finding::unexplained));

    let body = render(&findings, &crowded(&scan.dirs()));
    fs::create_dir_all(".lint").ok();
    fs::write(REPORT, &body).ok();
    if findings.is_empty() {
        print!("{body}");
    } else {
        eprint!("{body}");
        exit(1);
    }
}

/// Directories past the ≲10-files aim, most crowded first. A signal, never a failure.
fn crowded<'a>(dirs: &BTreeMap<&'a str, Vec<&str>>) -> Vec<(&'a str, usize)> {
    let mut over: Vec<(&str, usize)> = dirs
        .iter()
        .map(|(d, names)| (*d, names.iter().filter(|n| **n != "mod.rs").count()))
        .filter(|(_, n)| *n > prefix::AIM_FILES_PER_DIR)
        .collect();
    over.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    over
}

fn render(findings: &[Finding], crowded: &[(&str, usize)]) -> String {
    let mut body = String::new();
    for f in findings {
        body.push_str(&format!("{f}\n"));
    }
    if !crowded.is_empty() {
        body.push_str(&format!(
            "signal (not a failure): {} directories hold more than {} source files; \
             cluster them by responsibility when you next work there:\n",
            crowded.len(),
            prefix::AIM_FILES_PER_DIR
        ));
        for (d, n) in crowded {
            body.push_str(&format!("  {n:>3}  {d}/\n"));
        }
    }
    body.push_str(&match findings.len() {
        0 => "rules: ok\n".to_string(),
        n => format!("rules: FAILED, {n} finding(s)\n"),
    });
    body
}

#[cfg(test)]
mod lockfile_tests;
