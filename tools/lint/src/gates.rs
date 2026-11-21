//! Every gate `main` dispatches, one row each, and the one way a gate reports.
//!
//! A gate is a pure `verdict(base, args)` over the repository at `base`: what it
//! found and the report it renders, never printed or exited from inside. `main`
//! prints and exits for all of them ([`report`]), so the wiring test
//! (`gates_tests.rs`) can run every row's real entry point over a fixture it must
//! flag, and a gate that returns nothing, or a row that stops being reached, fails
//! a test instead of passing silently.

use std::fs;
use std::path::Path;
use std::process::exit;

const REPORT: &str = ".lint/report.txt";

/// What a gate found over one repository.
pub struct Verdict {
    /// One line per finding; the gate fails when there is any.
    pub findings: Vec<String>,
    /// The report: stdout on a pass, stderr on a failure, and `.lint/report.txt`.
    pub body: String,
    /// What to do about the findings: stderr only, after the body, on a failure.
    pub advice: String,
}

impl Verdict {
    /// The shape most gates share: `<name>: ok`, or `<name>: FAILED` and a line per
    /// finding, with `advice` (given the count) under it.
    pub fn listed(name: &str, findings: Vec<String>, advice: impl Fn(usize) -> String) -> Self {
        let head = if findings.is_empty() { "ok" } else { "FAILED" };
        let mut body = format!("{name}: {head}\n");
        for f in &findings {
            body.push_str(f);
            body.push('\n');
        }
        let advice = if findings.is_empty() {
            String::new()
        } else {
            advice(findings.len())
        };
        Verdict {
            findings,
            body,
            advice,
        }
    }
}

/// A gate: everything it finds in the repository at `base`, given `main`'s
/// arguments (the size gate takes the commit hook's explicit files).
pub type Check = fn(&Path, &[String]) -> Verdict;

/// Every gate, with the flag that selects it; `""` is the default, the size gate.
/// A new gate is one row here and a must-flag case in `gates_tests.rs`, whose
/// test fails naming any row without one.
pub const GATES: &[(Check, &str)] = &[
    // sorted: gates are independent; `main` runs the one its flag names
    (crate::components::verdict, "--components"),
    (crate::determinism::verdict, "--determinism"),
    (crate::layers::verdict, "--layers"),
    (crate::parity::verdict, "--parity"),
    (crate::rules::verdict, "--rules"),
    (crate::size::verdict, ""),
];

/// The flag of the one report that is not a row: `--dead-pub` (`main`).
pub const DEAD_PUB: &str = "--dead-pub";

/// The gate `args` selects: the row whose flag is among them, else the size gate.
/// `Err` when they name two (a gate's flag, [`DEAD_PUB`] or the comment-only
/// proof, twice over): `main`
/// runs one, and which would be a guess.
pub fn select(args: &[String]) -> Result<Check, String> {
    let named: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .filter(|a| {
            *a == DEAD_PUB
                || *a == crate::comments::proof::FLAG
                || GATES.iter().any(|(_, f)| !f.is_empty() && f == a)
        })
        .collect();
    if let [_, _, ..] = named.as_slice() {
        return Err(format!(
            "lint: {} name more than one gate; run one gate per call",
            named.join(" ")
        ));
    }
    let flag = named.first().copied().unwrap_or("");
    Ok(GATES
        .iter()
        .find(|(_, f)| *f == flag)
        .map_or(crate::size::verdict, |(check, _)| *check))
}

/// Print `verdict` as every gate does, mirror it to `.lint/report.txt`, and exit 1
/// when it found anything.
pub fn report(verdict: &Verdict) {
    fs::create_dir_all(".lint").ok();
    fs::write(REPORT, &verdict.body).ok();
    if verdict.findings.is_empty() {
        print!("{}", verdict.body);
        return;
    }
    eprint!("{}", verdict.body);
    eprint!("{}", verdict.advice);
    exit(1);
}

#[cfg(test)]
#[path = "gates_tests.rs"]
mod tests;
