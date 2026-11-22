//! Shared widget helpers for the editor panels.

use egui::emath::Numeric;
use std::ops::RangeInclusive;

/// A ranged `DragValue` that never rewrites a value it did not edit: egui's `range`
/// alone also clamps the value it shows.
pub trait DragRange: Sized {
    fn ranged<N: Numeric>(self, range: RangeInclusive<N>) -> Self;
}

impl DragRange for egui::DragValue<'_> {
    fn ranged<N: Numeric>(self, range: RangeInclusive<N>) -> Self {
        // lint: allow(drag-range) the one place that sets the range, with clamping off
        self.range(range).clamp_existing_to_range(false)
    }
}

/// An `egui::Slider` over `range` that never rewrites a value it did not edit: egui's
/// default `SliderClamping::Always` clamps the shown value too.
pub fn slider<N: Numeric>(value: &mut N, range: RangeInclusive<N>) -> egui::Slider<'_> {
    // lint: allow(drag-range) the one place that builds a slider, with clamping on edits only
    egui::Slider::new(value, range).clamping(egui::SliderClamping::Edits)
}

#[cfg(test)]
#[path = "widgets_tests.rs"]
mod tests;
