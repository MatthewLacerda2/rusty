//! The crate half of the layer table (#938): which external crates each module may
//! name. Each row in [`super::table`] lists its crates; [`policy`] holds what is
//! not one row's (the crates every module shares, the ones no module names, the
//! ones confined to one subtree, and the exceptions). `make layers` runs both
//! halves.

mod cargo;
mod check;
mod find;
mod members;
pub mod policy;
#[cfg(test)]
mod tests;

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

/// Check the modules' crates against the table and the manifests: the root
/// package's `Cargo.toml` and every member's under `root`.
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
    }
    out
}

/// The external crates the root package and every member under `root` depend on:
/// a crate a moved module took with it is declared only in its member's manifest.
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
