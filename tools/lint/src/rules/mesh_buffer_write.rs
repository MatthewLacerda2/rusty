use super::finding::Finding;
use super::scan::Scan;
use crate::code::bare_lines;

pub const RULE: &str = "mesh-buffer-write";
const HOMES: [&str; 2] = ["src/asset/", "src/components/mesh/geometry.rs"];

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

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.files
        .iter()
        .flat_map(|p| check(p, scan.source(p)))
        .collect()
}

#[cfg(test)]
#[path = "mesh_buffer_write_tests.rs"]
mod tests;
