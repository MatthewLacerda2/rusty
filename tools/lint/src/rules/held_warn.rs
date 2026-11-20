//! `held-warn`: code a model's import spreads over threads warns through
//! `rusty_core::warn_held!`, never `log::` (#2332).
//!
//! `asset::cook::par::map` runs each sub-mesh's work on a worker thread, which
//! carries neither `core::quiet::quietly`'s flag nor a test's per-thread log
//! capture, and whose warnings would land in whatever order the threads ran. So
//! the work runs `core::quiet::holding`, and the caller says what each item held
//! after the join, in item order; only `warn_held!` is held. A `log::warn!` (or
//! any other logging macro) in a module those closures reach escapes all three.
//! The modules are [`REACHED`], by module path (`crates/asset/src/` on disk); a
//! module a `par::map` closure comes to reach joins that list. `warn_held!`
//! logs as usual when nothing holds, so a whole module takes it at no cost. A
//! real need says why with `// lint: allow(held-warn) <why>`.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::{strip_comment, Open};

pub const RULE: &str = "held-warn";
/// The modules a `cook::par::map` closure reaches: a folder (ending in `/`) or a
/// file. The sub-mesh steps (`lightmap_uv`, `clusters`), the tangents and flat
/// normals a parse generates, the glTF mesh read with its clips, the `.obj`
/// piece read, and a cooked sub-mesh's digest and lay.
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
/// What such a module must not call: `warn_held!` names none of these.
const LOGGING: &[&str] = &[
    "log::", "log!", "warn!", "error!", "info!", "debug!", "trace!",
];

/// Every logging line of `source`, when `path` is a module [`REACHED`] names.
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

/// Does `code` call a logging macro, with no identifier character before it?
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

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(check)
}

#[cfg(test)]
#[path = "held_warn_tests.rs"]
mod tests;
