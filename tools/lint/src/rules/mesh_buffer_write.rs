//! `mesh-buffer-write`: a mesh's buffers are written in place through its
//! accessors (#2351).
//!
//! A mesh's vertex and index buffers are `rusty_asset::mesh_data::Shared`, held by
//! the import cache and every instance of a sub-mesh. `Shared::edit` copies a
//! buffer someone else holds, but only `MeshComponent::vertices_mut` /
//! `indices_mut` also renew the revision (so every renderer re-uploads) and give
//! the mesh a GPU slot of its own (so the edit is not drawn on its siblings). So a
//! line whose code (outside string literals) calls `.vertices.edit(` or
//! `.indices.edit(` is a finding outside the importer (`src/asset/`, which edits
//! buffers it owns alone, before any mesh holds them) and the accessors' home
//! (`src/components/mesh/geometry.rs`). Test code is held too: a test that skips
//! the revision tests a mesh no engine path makes. A text match: a call split
//! across lines (`.vertices\n.edit()`) is not seen. `tools/` is not scanned.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::bare_lines;

pub const RULE: &str = "mesh-buffer-write";
const HOMES: [&str; 2] = ["src/asset/", "src/components/mesh/geometry.rs"];

/// Every line of `path` writing a mesh buffer through `Shared::edit` directly.
pub fn check(path: &str, source: &str) -> Vec<Finding> {
    let at = crate::modules::module_path(path);
    if path.starts_with("tools/") || HOMES.iter().any(|h| at.starts_with(h)) {
        return Vec::new();
    }
    let code = bare_lines(source);
    (0..code.len())
        .filter(|&i| code[i].contains(".vertices.edit(") || code[i].contains(".indices.edit("))
        .map(|i| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message: "edit a mesh's buffers through `vertices_mut()` / `indices_mut()`, which \
                      renew its revision and give it a GPU slot of its own"
                .to_string(),
        })
        .collect()
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.files
        .iter()
        .flat_map(|p| check(p, scan.source(p)))
        .collect()
}

#[cfg(test)]
#[path = "mesh_buffer_write_tests.rs"]
mod tests;
