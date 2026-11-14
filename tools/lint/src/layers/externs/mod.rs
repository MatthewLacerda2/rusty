//! The crate half of the layer table (#938): which crates each member's manifest
//! depends on. Each row in [`super::table`] lists its external crates; [`policy`]
//! holds what is not one row's (the crates every module shares, the ones no module
//! names, the ones confined to one subtree, and the display crates no sim member
//! may take). `make layers` runs both halves.

mod cargo;
mod check;
mod find;
mod members;
pub mod policy;
#[cfg(test)]
mod tests;

pub use cargo::{dev_keys as manifest_dev_dependencies, keys as manifest_dependencies};
pub use find::flatten;

use super::scan::Tree;
use super::table::Layer;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// One file of `module` naming external crate `krate`, at `file:line`.
pub struct Use {
    pub module: String,
    pub krate: String,
    pub at: String,
}

/// Check the table against the manifests (the root package's `Cargo.toml` and
/// every member's under `root`) and the modules' code.
pub fn violations(layers: &[Layer], root: &Path, tree: &Tree) -> Vec<String> {
    let deps = match workspace_dependencies(root) {
        Ok(deps) => deps,
        Err(why) => return vec![why],
    };
    let uses = uses(tree, &deps.iter().cloned().collect());
    let mut out = check::all(layers, &policy::POLICY, &deps, &uses);
    for member in crate::modules::member_manifests(root) {
        let Some(module) = member.parent().and_then(|d| d.file_name()) else {
            continue;
        };
        let text = fs::read_to_string(&member).unwrap_or_default();
        let module = module.to_string_lossy();
        out.extend(members::stray(&module, &text, layers, &policy::POLICY));
        out.extend(members::dev_stray(&module, &text, layers));
        out.extend(members::display(&module, &text, layers, &policy::POLICY));
        out.extend(members::support(&module, &text));
    }
    out
}

/// The external crates the root package and every member under `root` depend on.
pub fn workspace_dependencies(root: &Path) -> Result<Vec<String>, String> {
    let manifest = root.join("Cargo.toml");
    let text = fs::read_to_string(&manifest)
        .map_err(|_| format!("NO_MANIFEST {} — cannot read it", manifest.display()))?;
    let mut deps = cargo::dependencies(&text);
    for member in crate::modules::member_manifests(root) {
        deps.extend(cargo::dependencies(
            &fs::read_to_string(member).unwrap_or_default(),
        ));
    }
    deps.sort();
    deps.dedup();
    Ok(deps)
}

/// Every external-crate reference in non-test module code.
fn uses(tree: &Tree, crates: &BTreeSet<String>) -> Vec<Use> {
    let mut out = Vec::new();
    for (file, content) in tree.live_files() {
        let Some(module) = tree.owner(file) else {
            continue;
        };
        let norm = file.to_string_lossy().into_owned();
        for r in find::crate_refs(content, crates) {
            out.push(Use {
                module: module.clone(),
                krate: r.krate,
                at: format!("{norm}:{}", r.line),
            });
        }
    }
    out
}
