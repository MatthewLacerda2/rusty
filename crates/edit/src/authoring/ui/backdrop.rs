use glam::Vec4;

use rusty_components::BackdropFilterComponent;
use rusty_scene::rules::sanitize::{non_negative, unit_color};

pub fn set_blur_radius(b: &mut BackdropFilterComponent, radius: f32) {
    b.blur_radius = non_negative(radius);
}

/// Alpha is the mix.
pub fn set_tint(b: &mut BackdropFilterComponent, tint: Vec4) {
    b.tint = unit_color(tint);
}

/// `1` unchanged, `0` greyscale.
pub fn set_saturation(b: &mut BackdropFilterComponent, saturation: f32) {
    b.saturation = non_negative(saturation);
}

pub fn set_brightness(b: &mut BackdropFilterComponent, brightness: f32) {
    b.brightness = non_negative(brightness);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ops_write_through_and_clamp() {
        let mut b = BackdropFilterComponent::default();
        set_blur_radius(&mut b, -1.0);
        assert_eq!(b.blur_radius, 0.0);
        set_tint(&mut b, Vec4::new(2.0, 0.5, -1.0, 0.25));
        assert_eq!(b.tint, Vec4::new(1.0, 0.5, 0.0, 0.25));
        set_saturation(&mut b, -2.0);
        set_brightness(&mut b, 0.4);
        assert_eq!((b.saturation, b.brightness), (0.0, 0.4));
    }
}
