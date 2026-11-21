//! `bind-group-index`: a `set_bind_group` names its group, never a number.
//!
//! The solid path's group order is by how often each group changes (the per-draw
//! group last), and it lives in one place: `render::gpu::groups` (`GLOBAL`,
//! `SCENE`, `MATERIAL`, `DRAW`, from `rusty_shadergen::groups`). A call that binds
//! at a number literal hides which group it means, so a reorder there leaves it
//! binding the wrong one (`crates/render/src/CLAUDE.md`, *Reuse `render::gpu`*). A
//! pass outside the solid path (post-FX, UI, the blits) names its own groups with
//! a one-line `const` beside the pass. Test code is not held.
//!
//! The first argument is read up to its `,`, across a line break, so a call
//! formatted over several lines is judged too; the finding sits on the call's line.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::bare_lines;

pub const RULE: &str = "bind-group-index";
const CALL: &str = "set_bind_group(";

/// Every call in a non-test line of an engine source that binds at a number.
pub fn check(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    if !crate::modules::module_path(path).starts_with("src/") {
        return Vec::new();
    }
    let lines = bare_lines(source);
    let mut found = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if test.get(i).copied().unwrap_or(false) {
            continue;
        }
        for (at, _) in line.match_indices(CALL) {
            let mut rest = vec![&line[at + CALL.len()..]];
            rest.extend(lines[i + 1..].iter().map(String::as_str));
            if is_number(first_argument(&rest)) {
                found.push(Finding {
                    path: path.to_string(),
                    line: i + 1,
                    rule: RULE,
                    message: "name the group, not its number: a solid-path pass binds \
                              `groups::GLOBAL`, `SCENE`, `MATERIAL` or `DRAW` \
                              (`render::gpu::groups`); any other pass a `const` of its own"
                        .to_string(),
                });
            }
        }
    }
    found
}

/// The text before the first `,` of what follows the call's `(`, trimmed, read
/// across the lines in `rest` until one holds code.
fn first_argument<'a>(rest: &[&'a str]) -> &'a str {
    rest.iter()
        .map(|l| l.split(',').next().unwrap_or("").trim())
        .find(|arg| !arg.is_empty())
        .unwrap_or("")
}

/// Is `arg` an integer literal (`1`, `0u32`, `2_u32`)?
fn is_number(arg: &str) -> bool {
    let digits = arg.trim_end_matches("u32").trim_end_matches('_');
    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit() || c == '_')
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|p, s| check(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "bind_group_index_tests.rs"]
mod tests;
