//! The crate half of the layer table (#938): which external crates each module may
//! name. Each row in [`super::table`] lists its crates; [`policy`] holds what is
//! not one row's (the crates every module shares, the ones no module names, the
//! ones confined to one subtree, and the exceptions). `make layers` runs both
//! halves.

mod cargo;
mod check;
mod find;
pub mod policy;
#[cfg(test)]
mod tests;

use super::graph::owner;
use super::scan;
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

/// Check the crates under `src` against the table and `Cargo.toml`.
pub fn violations(layers: &[Layer], manifest: &Path, src: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(manifest) else {
        return vec![format!(
            "NO_MANIFEST {} — cannot read it",
            manifest.display()
        )];
    };
    let deps = cargo::dependencies(&text);
    let uses = uses(src, &deps.iter().cloned().collect());
    check::all(layers, &policy::POLICY, &deps, &uses)
}

/// Every external-crate reference in non-test module code under `src`.
fn uses(src: &Path, crates: &BTreeSet<String>) -> Vec<Use> {
    let files = scan::walk(src);
    let test_only = super::gate::test_only_files(&files);
    let mut out = Vec::new();
    for file in files.iter().filter(|f| !test_only.contains(*f)) {
        let (Some(module), Ok(content)) = (owner(src, file), fs::read_to_string(file)) else {
            continue;
        };
        let norm = file.to_string_lossy().replace('\\', "/");
        for r in find::crate_refs(&content, crates) {
            out.push(Use {
                module: module.clone(),
                krate: r.krate,
                at: format!("{norm}:{}", r.line),
            });
        }
    }
    out
}
