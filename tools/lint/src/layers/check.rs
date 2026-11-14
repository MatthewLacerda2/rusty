//! The table checks, pure over (table, member crates, measured graph) so tests can
//! feed all three. Cargo already rejects a crate naming a sibling it does not
//! depend on, so the source is read only for what cargo accepts: a declared
//! dependency nothing names.

use super::graph::{self, Edge};
use super::table::Layer;

/// Every violation, in a stable order: table shape first, then the source.
pub fn all(layers: &[Layer], modules: &[String], edges: &[Edge]) -> Vec<String> {
    let mut out = Vec::new();
    declared(layers, modules, &mut out);
    order(layers, &mut out);
    sim_closed(layers, &mut out);
    stale(layers, edges, &mut out);
    out
}

/// Every module crate under `crates/` has exactly one row, and every row a crate.
fn declared(layers: &[Layer], modules: &[String], out: &mut Vec<String>) {
    for m in modules {
        if layers.iter().filter(|l| l.module == m).count() != 1 {
            out.push(format!(
                "UNDECLARED_MODULE crates/{m} — give it one row in the table"
            ));
        }
    }
    for l in layers {
        if !modules.iter().any(|m| m == l.module) {
            out.push(format!(
                "MISSING_MODULE {} — declared but crates/{0}/src is absent",
                l.module
            ));
        }
    }
}

/// Each dep is declared above its importer: the table reads bottom-up, and "below"
/// (what a dev-dependency may name) is an earlier row.
fn order(layers: &[Layer], out: &mut Vec<String>) {
    let rank = |m: &str| layers.iter().position(|l| l.module == m);
    for (i, l) in layers.iter().enumerate() {
        for &dep in l.deps {
            match rank(dep) {
                None => out.push(format!("UNKNOWN_DEP {} → {dep} — no such row", l.module)),
                Some(j) if j >= i => out.push(format!(
                    "CYCLE {} → {dep} — {dep} is declared below {0}; deps point up the table",
                    l.module
                )),
                _ => {}
            }
        }
    }
}

/// A sim module depends only on sim modules, so no chain of edges leaves the sim.
fn sim_closed(layers: &[Layer], out: &mut Vec<String>) {
    for l in layers.iter().filter(|l| l.sim) {
        for &dep in l.deps {
            if layers.iter().any(|d| d.module == dep && !d.sim) {
                out.push(format!(
                    "SIM_LEAK {} → {dep} — a sim module may depend only on sim modules",
                    l.module
                ));
            }
        }
    }
}

/// Every declared dep is named by its crate's live code: cargo builds an unused
/// dependency without a word, and one only tests name is a dev-dependency.
fn stale(layers: &[Layer], edges: &[Edge], out: &mut Vec<String>) {
    let seen = graph::summarize(edges);
    let used = |from: &str, to: &str| seen.get(from).is_some_and(|t| t.contains_key(to));
    for l in layers {
        for &dep in l.deps.iter().filter(|&&d| !used(l.module, d)) {
            out.push(format!(
                "STALE_DEP {} → {dep} — no live code names rusty_{}; drop it from the row \
                 and the manifest (a dev-dependency if only tests name it)",
                l.module,
                dep.replace('-', "_")
            ));
        }
    }
}
