//! `drag-range`: a ranged editor drag or slider never rewrites a value it did not
//! edit (#1158, #1444).
//!
//! egui's `DragValue::range` also clamps the value a field *shows*, so selecting an
//! entity whose value sits outside the range rewrites it. `editor::widgets::DragRange`
//! (`.ranged(..)`) turns that off; a `.range(` in a file under `src/editor` that
//! names `DragValue` is a finding. A file that never names `DragValue` is not scanned
//! for it, so another type's `.range(` is safe. `egui::Slider` clamps the same way by
//! default (`SliderClamping::Always`), so a `Slider::new(` under `src/editor` is a
//! finding too: `editor::widgets::slider` builds one that clamps only edits. Either
//! is excused by `// lint: allow(drag-range) <why>`; `src/editor/widgets.rs`, the
//! helpers themselves, carries its own allows.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "drag-range";

/// Every `.range(` code line of an editor file that uses `DragValue`, and every
/// `Slider::new(` code line of an editor file.
pub fn check(path: &str, source: &str) -> Vec<Finding> {
    if !path.starts_with("src/editor/") {
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

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(check)
}

#[cfg(test)]
#[path = "drag_range_tests.rs"]
mod tests;
