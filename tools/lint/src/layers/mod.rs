//! Layering lint (#724): the declared table in [`table`] equals the workspace's
//! dependency graph. Cargo holds the edges (a crate cannot name what it does not
//! depend on, and a cycle of `[dependencies]` does not resolve); this lint holds
//! what cargo cannot: one row per member crate, in bottom-up order, the sim mark
//! (no sim module depends on a platform one, or names a display crate, tests
//! included), and no declared edge left unused. The [`externs`] half holds each
//! member's manifest equal to its row, external crates included (#938).
//!
//! The unused-edge scan skips test code, by the one definition in `gate` that the
//! rules exempting test code (`make rules`) also read. The table also feeds
//! [`sim_dirs`] and [`platform_roots`], the only files the determinism gate lets opt
//! out of clippy's clock ban.
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

use crate::gates::Verdict;
/// One item's extent and what test code is, shared with the rules' test-code
/// classifier.
pub use extent::item_end;
/// The use-tree flattener, shared with the `api-parity` rule.
pub use externs::flatten as flatten_use;
/// A manifest's `[dependencies]` and `[dev-dependencies]` crate names, shared with
/// the rules' package scan.
pub use externs::{manifest_dependencies, manifest_dev_dependencies};
/// The lexical path folder, shared with the `orphan-module` rule.
pub use gate::normalise;
pub use gate::{is_test_gate, test_only_files};
use std::path::Path;

/// The sim modules' directories at their module paths (`src/<m>`, read as
/// `crates/<m>/src`: `crate::modules`), derived from the table's `sim` marks.
pub fn sim_dirs() -> Vec<String> {
    table::LAYERS
        .iter()
        .filter(|l| l.sim)
        .map(|l| format!("src/{}", l.module))
        .collect()
}

/// The platform modules' roots at their module paths (`src/<module>/mod.rs`, read
/// as `crates/<module>/src/lib.rs`): the only files that may opt out of the sim's
/// clock and RNG ban (#757).
pub fn platform_roots() -> Vec<String> {
    table::LAYERS
        .iter()
        .filter(|l| !l.sim)
        .map(|l| format!("src/{}/mod.rs", l.module))
        .collect()
}

/// The gate over the repository at `base`: its module graph and manifests
/// against the table.
pub fn verdict(base: &Path, _: &[String]) -> Verdict {
    let tree = scan::Tree::read(base);
    let edges = graph::edges(&tree);
    let mut violations = check::all(table::LAYERS, &graph::modules(&tree), &edges);
    violations.extend(externs::violations(table::LAYERS, base, &tree));
    Verdict::listed("layers", violations, |n| {
        format!(
            "\n{n} layering violation(s). The table is tools/lint/src/layers/table.rs: \
             keep it equal to the crates' Cargo.toml files, and move code down a layer rather than \
             let a sim crate depend on the platform.\n"
        )
    })
}
