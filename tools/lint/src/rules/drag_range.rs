//! is excused by `// lint: allow(drag-range) <why>`; `src/editor/widgets.rs`, the

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "drag-range";

pub fn check(path: &str, source: &str) -> Vec<Finding> {
    let at = crate::modules::module_path(path);
    if !at.starts_with("src/editor/") {
        return Vec::new();
    }
    let drags = source.contains("DragValue");
    code_lines(source)
        .iter()
        .enumerate()
        .filter_map(|(i, code)| {
            let message = if drags && code.contains(".range(") {
                "a bare DragValue `.range(` rewrites an out-of-range value on selection; \
                 use `.ranged(..)` from editor::widgets::DragRange"
            } else if code.contains("Slider::new(") {
                "a bare `Slider::new(` clamps an out-of-range value on selection; \
                 use editor::widgets::slider"
            } else {
                return None;
            };
            Some(Finding {
                path: path.to_string(),
                line: i + 1,
                rule: RULE,
                message: message.to_string(),
            })
        })
        .collect()
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(check)
}

#[cfg(test)]
#[path = "drag_range_tests.rs"]
mod tests;
