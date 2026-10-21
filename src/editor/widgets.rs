//! Shared widget helpers for the editor panels.

use egui::emath::Numeric;
use std::ops::RangeInclusive;

/// A ranged `DragValue` that never rewrites a value it did not edit.
///
/// egui's own `DragValue::range` also clamps the value a field *shows* (it defaults
/// to `clamp_existing_to_range(true)`), so merely selecting an entity whose value a
/// scene or script put outside the range would rewrite it (#1142, #1158). `ranged`
/// bounds only what the user drags or types. Every ranged `DragValue` under
/// `src/editor` goes through it; `make rules` (`drag-range`) flags a bare `.range(`.
pub trait DragRange: Sized {
    fn ranged<N: Numeric>(self, range: RangeInclusive<N>) -> Self;
}

impl DragRange for egui::DragValue<'_> {
    fn ranged<N: Numeric>(self, range: RangeInclusive<N>) -> Self {
        // lint: allow(drag-range) the one place that sets the range, with clamping off
        self.range(range).clamp_existing_to_range(false)
    }
}

#[cfg(test)]
#[path = "widgets_tests.rs"]
mod tests;
