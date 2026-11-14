//! Each member's manifest against its own row: the merged set
//! [`super::workspace_dependencies`] checks cannot see a crate one member declares
//! that only another member's row names. A member `crates/<m>` that is a module of the
//! table may depend on the sibling crates of its row's `deps` (as `rusty-<dep>`) and
//! on the external crates of its row's `uses`, the common list and its exceptions.
//! Its dev-dependencies may name a sibling crate only below its row ([`dev_stray`]):
//! tests are not in the table, but a cycle is.

use super::cargo::{dev_keys, keys};
use super::policy::Policy;
use crate::layers::table::Layer;

/// Every stray dependency in the manifest `text` of the member `module`.
pub fn stray(module: &str, text: &str, layers: &[Layer], policy: &Policy) -> Vec<String> {
    let Some(row) = layers.iter().find(|l| l.module == module) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for key in keys(text) {
        let ok = match key.strip_prefix("rusty_") {
            Some(dep) => row.deps.contains(&dep),
            None => {
                let k = key.as_str();
                row.crates.contains(&k)
                    || policy.common.contains(&k)
                    || (policy.exceptions.iter()).any(|x| (x.module, x.krate) == (module, k))
            }
        };
        if !ok {
            let name = key.replace('_', "-");
            out.push(format!(
                "STRAY_CRATE crates/{module}/Cargo.toml → {name} — not in {module}'s row; \
                 drop it, or add it to the row if {module}'s code names it"
            ));
        }
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

#[cfg(test)]
#[path = "members_tests.rs"]
mod tests;
