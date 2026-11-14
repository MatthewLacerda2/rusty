//! Each member's manifest against its own row: a member `crates/<m>` that is a
//! module of the table depends on exactly the sibling crates of its row's `deps` (as
//! `rusty-<dep>`) and exactly the external crates of its row's `uses`, plus any of
//! the common list. Cargo then holds the code to the manifest, so the row is what
//! the crate can name. Its dev-dependencies may name a sibling crate only below its
//! row ([`dev_stray`]): cargo accepts a dev-dependency cycle, the table does not.
//! A sim member names no display crate at all ([`display`]).

use super::cargo::{dev_keys, keys};
use super::policy::Policy;
use crate::layers::table::Layer;

/// Every dependency in the manifest `text` of the member `module` that its row
/// lacks, and every one its row names that the manifest lacks.
pub fn stray(module: &str, text: &str, layers: &[Layer], policy: &Policy) -> Vec<String> {
    let Some(row) = layers.iter().find(|l| l.module == module) else {
        return Vec::new();
    };
    let declared = keys(text);
    let mut out = Vec::new();
    for key in &declared {
        let ok = match key.strip_prefix("rusty_") {
            Some(dep) => row.deps.contains(&dep),
            None => row.crates.contains(&key.as_str()) || policy.common.contains(&key.as_str()),
        };
        if !ok {
            let name = key.replace('_', "-");
            out.push(format!(
                "STRAY_CRATE crates/{module}/Cargo.toml → {name} — not in {module}'s row; \
                 drop it, or add it to the row if {module}'s code names it"
            ));
        }
    }
    let wanted = (row.deps.iter().map(|d| format!("rusty_{d}")))
        .chain(row.crates.iter().map(|k| k.to_string()));
    for key in wanted.filter(|k| !declared.contains(k)) {
        let name = key.replace('_', "-");
        out.push(format!(
            "MISSING_CRATE crates/{module}/Cargo.toml → {name} — {module}'s row names it but \
             its manifest does not depend on it; add the dependency or drop it from the row"
        ));
    }
    out
}

/// Every sibling crate the `[dev-dependencies]` of member `module` may not name: itself,
/// or a module at or above its row (the table is acyclic, so a row below never reaches
/// back). A member that is no module of the table (`rusty-test-support`) is no row's,
/// so any member may take it.
pub fn dev_stray(module: &str, text: &str, layers: &[Layer]) -> Vec<String> {
    let Some(at) = layers.iter().position(|l| l.module == module) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for key in dev_keys(text) {
        let Some(dep) = key.strip_prefix("rusty_") else {
            continue;
        };
        let Some(row) = layers
            .iter()
            .position(|l| l.module.replace('-', "_") == dep)
        else {
            continue;
        };
        let dep = layers[row].module;
        if row < at {
            continue;
        }
        out.push(format!(
            "STRAY_DEV_CRATE crates/{module}/Cargo.toml → rusty-{dep} — a dev-dependency on \
             a module that is not below {module} in the layer table makes a cycle; move the \
             test to `tests/` or to {dep}'s own tests"
        ));
    }
    out
}

/// Every display crate (`policy.display`) the manifest of a sim member `module`
/// names, as a dependency or a dev-dependency: the sim and its tests run with no
/// GPU and no window.
pub fn display(module: &str, text: &str, layers: &[Layer], policy: &Policy) -> Vec<String> {
    if !layers.iter().any(|l| l.module == module && l.sim) {
        return Vec::new();
    }
    let mut named = keys(text);
    named.extend(dev_keys(text));
    named.sort();
    named.dedup();
    named
        .iter()
        .filter(|k| policy.display.contains(&k.as_str()))
        .map(|k| {
            format!(
                "WRONG_DIRECTION crates/{module}/Cargo.toml → {} — a sim crate never depends \
                 on the display layer, not even in tests; move the type to the sim side \
                 (components / scene / core) and let the display crate import it",
                k.replace('_', "-")
            )
        })
        .collect()
}

#[cfg(test)]
#[path = "members_tests.rs"]
mod tests;
