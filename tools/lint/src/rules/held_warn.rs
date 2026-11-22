//! real need says why with `// lint: allow(held-warn) <why>`.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::{strip_comment, Open};

pub const RULE: &str = "held-warn";
pub const REACHED: &[&str] = &[
    "src/asset/clusters.rs",
    "src/asset/cook/sub.rs",
    "src/asset/flat_normals.rs",
    "src/asset/gltf/anim.rs",
    "src/asset/gltf/import.rs",
    "src/asset/lightmap_uv/",
    "src/asset/obj_import.rs",
    "src/asset/tangents.rs",
];
const LOGGING: &[&str] = &[
    "log::", "log!", "warn!", "error!", "info!", "debug!", "trace!",
];

pub fn check(path: &str, source: &str) -> Vec<Finding> {
    let at = crate::modules::module_path(path);
    let reached = REACHED.iter().any(|r| match r.ends_with('/') {
        true => at.starts_with(r),
        false => at == *r,
    });
    if !reached {
        return Vec::new();
    }
    let mut open = Open::None;
    let mut found = Vec::new();
    for (i, line) in source.lines().enumerate() {
        if logs(&strip_comment(line, &mut open)) {
            found.push(Finding {
                path: path.to_string(),
                line: i + 1,
                rule: RULE,
                message: "this module runs on `cook::par`'s worker threads: warn with \
                          `rusty_core::warn_held!`, which the caller says in item order"
                    .to_string(),
            });
        }
    }
    found
}

fn logs(code: &str) -> bool {
    LOGGING.iter().any(|name| {
        code.match_indices(name).any(|(at, _)| {
            !code[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_')
        })
    })
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(check)
}

#[cfg(test)]
#[path = "held_warn_tests.rs"]
mod tests;
