//! `drag-range`: a ranged editor drag never rewrites a value it did not edit (#1158).
//!
//! egui's `DragValue::range` also clamps the value a field *shows*, so selecting an
//! entity whose value sits outside the range rewrites it. `editor::widgets::DragRange`
//! (`.ranged(..)`) turns that off; a `.range(` in a file under `src/editor` that
//! names `DragValue` is a finding unless `// lint: allow(drag-range) <why>` excuses it.
//! `src/editor/widgets.rs`, the helper itself, carries its own allow. A file that
//! never names `DragValue` is not scanned, so another type's `.range(` is safe.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "drag-range";

/// Every `.range(` code line of an editor file that uses `DragValue`.
pub fn check(path: &str, source: &str) -> Vec<Finding> {
    if !path.starts_with("src/editor/") || !source.contains("DragValue") {
        return Vec::new();
    }
    code_lines(source)
        .iter()
        .enumerate()
        .filter(|(_, code)| code.contains(".range("))
        .map(|(i, _)| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message: "a bare DragValue `.range(` rewrites an out-of-range value on selection; \
                      use `.ranged(..)` from editor::widgets::DragRange"
                .to_string(),
        })
        .collect()
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(check)
}

#[cfg(test)]
#[path = "drag_range_tests.rs"]
mod tests;
