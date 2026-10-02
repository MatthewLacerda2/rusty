//! The table checks, pure over (table, measured graph) so tests can feed both.

use super::graph::{self, Edge};
use super::table::{Exception, Layer};

/// Every violation, in a stable order: table shape first, then the source.
pub fn all(
    layers: &[Layer],
    peers: &[(&str, &str)],
    exceptions: &[Exception],
    modules: &[String],
    edges: &[Edge],
) -> Vec<String> {
    let mut out = Vec::new();
    declared(layers, modules, &mut out);
    order(layers, peers, &mut out);
    sim_closed(layers, &mut out);
    source(layers, exceptions, edges, &mut out);
    out
}

/// Every module under `src/` has exactly one row, and every row exists.
fn declared(layers: &[Layer], modules: &[String], out: &mut Vec<String>) {
    for m in modules {
        if layers.iter().filter(|l| l.module == m).count() != 1 {
            out.push(format!(
                "UNDECLARED_MODULE src/{m} — give it one row in the table"
            ));
        }
    }
    for l in layers {
        if !modules.iter().any(|m| m == l.module) {
            out.push(format!(
                "MISSING_MODULE {} — declared but src/{0} is absent",
                l.module
            ));
        }
    }
}

/// Each dep is declared above its importer (or is a peer): the table is a
/// topological order, so the declared graph — and the code under it — is acyclic.
fn order(layers: &[Layer], peers: &[(&str, &str)], out: &mut Vec<String>) {
    let rank = |m: &str| layers.iter().position(|l| l.module == m);
    for (i, l) in layers.iter().enumerate() {
        for &dep in l.deps {
            let is_peer = peers
                .iter()
                .any(|&(a, b)| (a, b) == (l.module, dep) || (b, a) == (l.module, dep));
            match rank(dep) {
                None => out.push(format!("UNKNOWN_DEP {} → {dep} — no such row", l.module)),
                Some(j) if j >= i && !is_peer => out.push(format!(
                    "CYCLE {} → {dep} — {dep} is declared below {0}; deps point up the table",
                    l.module
                )),
                _ => {}
            }
        }
    }
}

/// A sim module imports only sim modules, so no chain of edges leaves the sim.
fn sim_closed(layers: &[Layer], out: &mut Vec<String>) {
    for l in layers.iter().filter(|l| l.sim) {
        for &dep in l.deps {
            if layers.iter().any(|d| d.module == dep && !d.sim) {
                out.push(format!(
                    "SIM_LEAK {} → {dep} — a sim module may import only sim modules",
                    l.module
                ));
            }
        }
    }
}

/// The source's edges match the table exactly, exceptions aside.
fn source(layers: &[Layer], exceptions: &[Exception], edges: &[Edge], out: &mut Vec<String>) {
    let excepted = |from: &str, to: &str| exceptions.iter().any(|x| (x.from, x.to) == (from, to));
    for e in edges {
        let allowed = layers
            .iter()
            .any(|l| l.module == e.from && l.deps.contains(&e.to.as_str()));
        if !allowed && !excepted(&e.from, &e.to) {
            out.push(format!(
                "UNDECLARED_DEP {} `{} → {}` — not in {0}'s row",
                e.at, e.from, e.to
            ));
        }
    }
    let seen = graph::summarize(edges);
    let used = |from: &str, to: &str| seen.get(from).is_some_and(|t| t.contains_key(to));
    for l in layers {
        for &dep in l.deps.iter().filter(|&&d| !used(l.module, d)) {
            out.push(format!(
                "STALE_DEP {} → {dep} — nothing references it; drop it",
                l.module
            ));
        }
    }
    for x in exceptions {
        if !used(x.from, x.to) {
            out.push(format!(
                "STALE_EXCEPTION {} → {} — fixed; drop it and close #{}",
                x.from, x.to, x.issue
            ));
        }
    }
}
