//! `drawn-mode`: the renderer reads what a light is drawn as from one place
//! (#2522), `render::clusters::unbaked::drawn_mode`.
//!
//! A `Baked` light no bake holds is previewed live in the editor and dev builds
//! and drawn from its bake in a shipped player; `Scene::light_mode` and
//! `LightComponent::effective_mode` know neither. A pass that reads either draws
//! such a light dark where every other pass draws it lit, so outside the entry
//! point no renderer line calls `light_mode(` or `effective_mode(`. Test code is
//! not scanned (a test may check what the scene says), nor the bake crates, which
//! bake from a light's own mode.
//!
//! A bare `.mode` read is not held: the field name is shared by the renderer's
//! other settings (exposure, depth of field, a particle emitter's render mode), so
//! text alone cannot tell a light's from theirs. Review holds that one.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "drawn-mode";

/// The renderer, at its module path.
const SCOPE: &str = "src/render/";

/// The one renderer file that reads a light's mode, through `drawn_mode`.
const ENTRY: &str = "src/render/clusters/unbaked.rs";

/// The calls that read a light's mode short of what the frame draws.
const READS: [&str; 2] = ["light_mode(", "effective_mode("];

/// Every renderer line of `source` that calls one of [`READS`]; `test` marks its
/// test lines.
pub fn check(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    let at = crate::modules::module_path(path);
    if !at.starts_with(SCOPE) || at == ENTRY {
        return Vec::new();
    }
    let is_test = |i: usize| test.get(i).copied().unwrap_or(false);
    (code_lines(source).iter().enumerate())
        .filter(|&(i, line)| !is_test(i) && reads_mode(line.as_ref()))
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

/// Whether `code` calls one of [`READS`] itself, not a longer name ending in it.
fn reads_mode(code: &str) -> bool {
    READS.iter().any(|read| {
        code.match_indices(read).any(|(at, _)| {
            !(code[..at].chars().next_back()).is_some_and(|c| c.is_alphanumeric() || c == '_')
        })
    })
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|p, s| check(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "drawn_mode_tests.rs"]
mod tests;
