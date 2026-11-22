use glam::Vec4;

use rusty_components::{MaskComponent, RectMaskComponent};
use rusty_scene::rules::sanitize::{finite4, non_negative};

/// Negative values grow the clip past the rect.
pub fn set_padding(m: &mut RectMaskComponent, padding: Vec4) {
    m.padding = finite4(padding);
}

pub fn set_feather(m: &mut RectMaskComponent, feather: f32) {
    m.feather = non_negative(feather);
}

pub fn set_show_mask_graphic(m: &mut MaskComponent, show: bool) {
    m.show_mask_graphic = show;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_writes_through() {
        let mut m = RectMaskComponent::default();
        set_padding(&mut m, Vec4::new(1.0, -2.0, 3.0, 4.0));
        assert_eq!(m.padding, Vec4::new(1.0, -2.0, 3.0, 4.0));
        set_feather(&mut m, -3.0);
        assert_eq!(m.feather, 0.0);
        set_feather(&mut m, 6.0);
        assert_eq!(m.feather, 6.0);
    }

    #[test]
    fn mask_graphic_toggle_writes_through() {
        let mut m = MaskComponent::default();
        assert!(m.show_mask_graphic);
        set_show_mask_graphic(&mut m, false);
        assert!(!m.show_mask_graphic);
    }
}
