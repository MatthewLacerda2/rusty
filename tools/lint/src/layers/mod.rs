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
pub use extent::item_end;
pub use externs::flatten as flatten_use;
pub use externs::{manifest_dependencies, manifest_dev_dependencies};
pub use gate::normalise;
pub use gate::{is_test_gate, test_only_files};
use std::path::Path;

pub fn sim_dirs() -> Vec<String> {
    table::LAYERS
        .iter()
        .filter(|l| l.sim)
        .map(|l| format!("src/{}", l.module))
        .collect()
}

pub fn platform_roots() -> Vec<String> {
    table::LAYERS
        .iter()
        .filter(|l| !l.sim)
        .map(|l| format!("src/{}/mod.rs", l.module))
        .collect()
}

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
