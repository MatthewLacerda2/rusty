//! Each member's manifest against its own row: the merged set
//! [`super::workspace_dependencies`] checks cannot see a crate one member declares
//! that only another member's row names. A member `crates/<m>` that is a module of the
//! table may depend on the sibling crates of its row's `deps` (as `rusty-<dep>`) and
//! on the external crates of its row's `uses`, the common list and its exceptions.

use super::cargo::keys;
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

#[cfg(test)]
#[path = "members_tests.rs"]
mod tests;
