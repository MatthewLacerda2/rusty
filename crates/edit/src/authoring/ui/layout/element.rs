//! A negative size override clears it, as Unity reads `-1` as "unset".

use rusty_components::{LayoutAxisFit, LayoutElementComponent};
use rusty_scene::rules::sanitize::optional_non_negative;

rusty_core::named_enum! {
    /// In `LayoutElementComponent::overrides`' order, which `size` indexes by discriminant.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum SizeKind {
        Min = "Min",
        Preferred = "Preferred",
        Flexible = "Flexible",
    }
}

pub fn set_ignore_layout(le: &mut LayoutElementComponent, ignore: bool) {
    le.ignore_layout = ignore;
}

pub fn set_size(
    le: &mut LayoutElementComponent,
    kind: SizeKind,
    width: Option<f32>,
    height: Option<f32>,
) {
    let (w, h) = (optional_non_negative(width), optional_non_negative(height));
    match kind {
        SizeKind::Min => (le.min_width, le.min_height) = (w, h),
        SizeKind::Preferred => (le.preferred_width, le.preferred_height) = (w, h),
        SizeKind::Flexible => (le.flexible_width, le.flexible_height) = (w, h),
    }
}

pub fn size(le: &LayoutElementComponent, kind: SizeKind) -> (Option<f32>, Option<f32>) {
    let i = kind as usize;
    (le.overrides(0)[i], le.overrides(1)[i])
}

pub fn set_fit(
    le: &mut LayoutElementComponent,
    horizontal: LayoutAxisFit,
    vertical: LayoutAxisFit,
) {
    le.horizontal_fit = horizontal;
    le.vertical_fit = vertical;
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
