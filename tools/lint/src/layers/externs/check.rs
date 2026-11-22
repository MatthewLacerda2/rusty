use super::policy::Policy;
use super::Use;
use crate::layers::table::Layer;

pub fn all(layers: &[Layer], policy: &Policy, deps: &[String], uses: &[Use]) -> Vec<String> {
    let mut out = Vec::new();
    accounted(layers, policy, deps, &mut out);
    confined(policy, uses, &mut out);
    stale(layers, policy, uses, &mut out);
    out
}

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
    let named = named.chain(policy.display.iter().copied());
    for k in named {
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

fn confined(policy: &Policy, uses: &[Use], out: &mut Vec<String>) {
    for u in uses {
        let k = u.krate.as_str();
        let path = crate::modules::module_path(u.at.split(':').next().unwrap_or(""));
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

fn stale(layers: &[Layer], policy: &Policy, uses: &[Use], out: &mut Vec<String>) {
    let used = |m: &str, k: &str| uses.iter().any(|u| u.module == m && u.krate == k);
    for l in layers {
        for k in l.crates.iter().filter(|k| !used(l.module, k)) {
            out.push(format!(
                "STALE_CRATE {} → {k} — nothing in crates/{0}/src names it; drop it from the row and the manifest",
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
}
