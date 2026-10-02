//! src/scene/authoring/layout_element.rs — Shared LayoutElement-authoring ops (#421).
//!
//! The ONE place the engine mutates an entity's first-class
//! `LayoutElementComponent`. The editor's Layout Element card and the Lua
//! `LayoutElement.*` namespace both route every write through these. A size
//! override is `None` (use the content's value) or a non-negative number — a
//! negative one clears it, as Unity reads `-1` as "unset".
//!
//! Pure.

use crate::components::{LayoutAxisFit, LayoutElementComponent};

/// Which of an element's three layout sizes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeKind {
    Min,
    Preferred,
    Flexible,
}

/// Every fit mode with its name.
pub const FITS: [(LayoutAxisFit, &str); 3] = [
    (LayoutAxisFit::Unconstrained, "Unconstrained"),
    (LayoutAxisFit::MinSize, "MinSize"),
    (LayoutAxisFit::PreferredSize, "PreferredSize"),
];

/// Set whether its parent's layout group skips it.
pub fn set_ignore_layout(le: &mut LayoutElementComponent, ignore: bool) {
    le.ignore_layout = ignore;
}

/// Set the `kind` override's width and height (`None` or negative: unset).
pub fn set_size(
    le: &mut LayoutElementComponent,
    kind: SizeKind,
    width: Option<f32>,
    height: Option<f32>,
) {
    let (w, h) = (valid(width), valid(height));
    match kind {
        SizeKind::Min => (le.min_width, le.min_height) = (w, h),
        SizeKind::Preferred => (le.preferred_width, le.preferred_height) = (w, h),
        SizeKind::Flexible => (le.flexible_width, le.flexible_height) = (w, h),
    }
}

/// The `kind` override's `(width, height)`.
pub fn size(le: &LayoutElementComponent, kind: SizeKind) -> (Option<f32>, Option<f32>) {
    let i = kind as usize;
    (le.overrides(0)[i], le.overrides(1)[i])
}

/// Set the content fitter along each axis.
pub fn set_fit(
    le: &mut LayoutElementComponent,
    horizontal: LayoutAxisFit,
    vertical: LayoutAxisFit,
) {
    le.horizontal_fit = horizontal;
    le.vertical_fit = vertical;
}

fn valid(v: Option<f32>) -> Option<f32> {
    v.filter(|v| *v >= 0.0 && v.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_sizes_clear_the_override() {
        let mut le = LayoutElementComponent::default();
        set_size(&mut le, SizeKind::Preferred, Some(40.0), Some(-1.0));
        assert_eq!(size(&le, SizeKind::Preferred), (Some(40.0), None));
        set_size(&mut le, SizeKind::Flexible, None, Some(2.0));
        assert_eq!((le.flexible_width, le.flexible_height), (None, Some(2.0)));
    }
}
