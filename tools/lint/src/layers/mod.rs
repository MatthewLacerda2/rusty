//! Layering lint (#724): the source's `crate::<module>` references must match the
//! declared table in [`table`] — no undeclared dep, no stale one, no cycle, and no
//! sim module importing a platform one, directly or through another module.
//!
//! Test code is exempt, as in the other guards. The table also feeds
//! [`sim_dirs`], the one sim set the determinism and direction guards scan.
//!
//! Usage: `cargo run --manifest-path tools/lint/Cargo.toml -- --layers`
//! Exit code 1 on any violation; report mirrored to `.lint/report.txt`.

mod check;
mod graph;
mod scan;
mod table;
#[cfg(test)]
mod tests;

use std::fs;
use std::path::Path;
use std::process::exit;

const REPORT: &str = ".lint/report.txt";

/// The sim modules' directories, derived from the table's `sim` marks.
pub fn sim_dirs() -> Vec<String> {
    table::LAYERS
        .iter()
        .filter(|l| l.sim)
        .map(|l| format!("src/{}", l.module))
        .collect()
}

/// Entry point: measure the module graph under `src/` and check it against the table.
pub fn run() {
    let src = Path::new("src");
    let edges = graph::edges(src);
    let violations = check::all(
        table::LAYERS,
        table::PEERS,
        table::EXCEPTIONS,
        &graph::modules(src),
        &edges,
    );
    report(&violations);
    if violations.is_empty() {
        println!("layers: ok");
    } else {
        exit(1);
    }
}

fn report(violations: &[String]) {
    let mut body = String::from(if violations.is_empty() {
        "layers: ok\n"
    } else {
        "layers: FAILED\n"
    });
    for v in violations {
        body.push_str(v);
        body.push('\n');
    }
    fs::create_dir_all(".lint").ok();
    fs::write(REPORT, &body).ok();
    if !violations.is_empty() {
        eprint!("{body}");
        eprintln!(
            "\n{} layering violation(s). The table is tools/lint/src/layers/table.rs: \
             move the code down a layer, or (if the new edge is right) declare it there.",
            violations.len()
        );
    }
}
