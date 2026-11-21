//! `drawn-mode`: the renderer reads what a light is drawn as from one place
//! (#2522), `render::clusters::unbaked::drawn_mode`.
//!
//! A `Baked` light no bake holds is previewed live in the editor and dev builds
//! and drawn from its bake in a shipped player; `Scene::light_mode` knows neither.
//! A pass that reads `light_mode(` draws such a light dark where every other pass
//! draws it lit, so outside the entry point no renderer line calls it. Test code
//! is not scanned (a test may check what the scene says), nor the bake crates,
//! which bake from a light's own mode.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "drawn-mode";

/// The renderer, at its module path.
const SCOPE: &str = "src/render/";

/// The one renderer file that reads `Scene::light_mode`, through `drawn_mode`.
const ENTRY: &str = "src/render/clusters/unbaked.rs";

/// Every renderer line of `source` that calls `light_mode(`; `test` marks its
/// test lines.
pub fn check(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    let at = crate::modules::module_path(path);
    if !at.starts_with(SCOPE) || at == ENTRY {
        return Vec::new();
    }
    let is_test = |i: usize| test.get(i).copied().unwrap_or(false);
    (code_lines(source).iter().enumerate())
        .filter(|&(i, line)| !is_test(i) && calls_light_mode(line.as_ref()))
        .map(|(i, _)| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message: "reads a light's mode past what the frame draws it as: read \
                      `crate::clusters::drawn_mode` (it previews a `Baked` light no bake \
                      holds yet)"
                .to_string(),
        })
        .collect()
}

/// Whether `code` calls `light_mode(` itself, not a longer name ending in it.
fn calls_light_mode(code: &str) -> bool {
    code.match_indices("light_mode(").any(|(at, _)| {
        !(code[..at].chars().next_back()).is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|p, s| check(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "drawn_mode_tests.rs"]
mod tests;
