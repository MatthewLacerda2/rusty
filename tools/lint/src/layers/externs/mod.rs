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

pub struct Use {
    pub module: String,
    pub krate: String,
    pub at: String,
}

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

fn uses(tree: &Tree, crates: &BTreeSet<String>) -> Vec<Use> {
    let mut out = Vec::new();
    for (file, content) in tree.live_files() {
        let Some(module) = tree.owner(file) else {
            continue;
        };
        let at = file.strip_prefix(&tree.root).unwrap_or(file);
        let norm = at.to_string_lossy().into_owned();
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
