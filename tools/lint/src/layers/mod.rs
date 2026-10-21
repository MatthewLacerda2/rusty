//! Layering lint (#724): the source's `crate::<module>` references must match the
//! declared table in [`table`] — no undeclared dep, no stale one, no cycle, and no
//! sim module importing a platform one, directly or through another module. The
//! [`externs`] half checks each module's external crates against its row (#938).
//!
//! Test code is exempt, by the one definition in `gate` that the rules exempting
//! test code (`make rules`) also read; the direction and determinism guards scan
//! it, since the sim → display arrow and the clock ban hold in tests too. The table also feeds [`sim_dirs`], the one sim set the direction
//! guard scans, and [`platform_roots`], the only files the determinism gate lets
//! opt out of clippy's clock ban.
//!
//! Usage: `cargo run --manifest-path tools/lint/Cargo.toml -- --layers`
//! Exit code 1 on any violation; report mirrored to `.lint/report.txt`.

mod check;
mod extent;
mod externs;
mod gate;
mod graph;
mod scan;
mod table;
#[cfg(test)]
mod tests;

/// One item's extent and what test code is, shared with the rules' test-code
/// classifier.
pub use extent::item_end;
pub use gate::{is_test_gate, test_only_files};
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

/// The platform modules' roots (`src/<module>/mod.rs`): the only files that may opt
/// out of the sim's clock and RNG ban (#757).
pub fn platform_roots() -> Vec<String> {
    table::LAYERS
        .iter()
        .filter(|l| !l.sim)
        .map(|l| format!("src/{}/mod.rs", l.module))
        .collect()
}

/// Entry point: measure the module graph under `src/` and check it against the table.
pub fn run() {
    let tree = scan::Tree::read(Path::new("src"));
    let edges = graph::edges(&tree);
    let mut violations = check::all(
        table::LAYERS,
        table::PEERS,
        table::EXCEPTIONS,
        &graph::modules(&tree),
        &edges,
    );
    violations.extend(externs::violations(
        table::LAYERS,
        Path::new("Cargo.toml"),
        &tree,
    ));
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
             move the code down a layer, or (if the new edge or crate use is right) declare it there.",
            violations.len()
        );
    }
}
