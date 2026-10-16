//! The crate checks, pure over (table, policy, `Cargo.toml`'s crates, measured uses).

use super::policy::Policy;
use super::Use;
use crate::layers::table::Layer;

/// Every violation: the table's shape against `Cargo.toml` first, then the source.
pub fn all(layers: &[Layer], policy: &Policy, deps: &[String], uses: &[Use]) -> Vec<String> {
    let mut out = Vec::new();
    accounted(layers, policy, deps, &mut out);
    source(layers, policy, uses, &mut out);
    stale(layers, policy, uses, &mut out);
    out
}

/// Every dependency is common, unscoped or named by a row, and every crate the
/// table names is a dependency.
fn accounted(layers: &[Layer], policy: &Policy, deps: &[String], out: &mut Vec<String>) {
    let in_rows = |k: &str| layers.iter().any(|l| l.crates.contains(&k));
    let common = |k: &str| policy.common.contains(&k);
    let unscoped = |k: &str| policy.unscoped.iter().any(|&(u, _)| u == k);
    for d in deps {
        if !common(d) && !unscoped(d) && !in_rows(d) {
            out.push(format!(
                "UNCLAIMED_CRATE {d} — Cargo.toml depends on it; name it in the rows that use it"
            ));
        }
    }
    let named = layers.iter().flat_map(|l| l.crates.iter().copied());
    let named = named.chain(policy.common.iter().copied());
    let named = named.chain(policy.unscoped.iter().map(|&(u, _)| u));
    let named = named.chain(policy.confined.iter().map(|c| c.krate));
    for k in named.chain(policy.exceptions.iter().map(|x| x.krate)) {
        if !deps.iter().any(|d| d == k) {
            out.push(format!(
                "UNKNOWN_CRATE {k} — not a dependency in Cargo.toml"
            ));
        }
    }
    for l in layers {
        for k in l.crates.iter().filter(|k| common(k) || unscoped(k)) {
            out.push(format!(
                "REDUNDANT_CRATE {} → {k} — it is common or unscoped; drop it from the row",
                l.module
            ));
        }
    }
}

/// Each use is in its module's row (or common, or excepted), and inside its
/// crate's confinement.
fn source(layers: &[Layer], policy: &Policy, uses: &[Use], out: &mut Vec<String>) {
    for u in uses {
        let k = u.krate.as_str();
        let listed = layers
            .iter()
            .any(|l| l.module == u.module && l.crates.contains(&k));
        let excepted = policy
            .exceptions
            .iter()
            .any(|x| (x.module, x.krate) == (u.module.as_str(), k));
        if !listed && !excepted && !policy.common.contains(&k) {
            out.push(format!(
                "UNDECLARED_CRATE {} `{} → {k}` — not in {1}'s row",
                u.at, u.module
            ));
        }
        let path = u.at.split(':').next().unwrap_or("");
        for c in policy.confined.iter().filter(|c| c.krate == k) {
            if !c.within.iter().any(|w| path.starts_with(&format!("{w}/"))) {
                out.push(format!(
                    "CONFINED_CRATE {} `{k}` — only {} may name it",
                    u.at,
                    c.within.join(", ")
                ));
            }
        }
    }
}

/// The table names nothing the code no longer uses.
fn stale(layers: &[Layer], policy: &Policy, uses: &[Use], out: &mut Vec<String>) {
    let used = |m: &str, k: &str| uses.iter().any(|u| u.module == m && u.krate == k);
    for l in layers {
        for k in l.crates.iter().filter(|k| !used(l.module, k)) {
            out.push(format!(
                "STALE_CRATE {} → {k} — nothing in src/{0} names it; drop it",
                l.module
            ));
        }
    }
    for k in policy
        .common
        .iter()
        .filter(|k| !uses.iter().any(|u| u.krate == **k))
    {
        out.push(format!("STALE_CRATE common → {k} — no module names it"));
    }
    for x in policy
        .exceptions
        .iter()
        .filter(|x| !used(x.module, x.krate))
    {
        out.push(format!(
            "STALE_EXCEPTION {} → {} — fixed; drop it and close #{}",
            x.module, x.krate, x.issue
        ));
    }
}
