//! The fill origin always suits the fill method (Unity's per-method origin enums, folded into one).

use glam::Vec4;

use crate::authoring::ui::look::sanitize_gradient;
use rusty_asset::MapRef;
use rusty_components::{FillMethod, FillOrigin, ImageComponent, ImageType, UiBlend, UiGradient};
use rusty_scene::rules::sanitize::{finite4, unit, unit_color};

/// Display space, straight alpha.
pub fn set_color(i: &mut ImageComponent, color: Vec4) {
    i.color = unit_color(color);
}

pub fn set_texture(i: &mut ImageComponent, path: Option<String>) {
    i.texture = path.filter(|p| !p.is_empty()).map(MapRef::from);
}

pub fn set_image_type(i: &mut ImageComponent, image_type: ImageType) {
    i.image_type = image_type;
}

/// Texels: left, bottom, right, top.
pub fn set_border(i: &mut ImageComponent, border: Vec4) {
    i.border = finite4(border).max(Vec4::ZERO);
}

pub fn set_fill_method(i: &mut ImageComponent, method: FillMethod) {
    i.fill_method = method;
    i.fill_origin = fit_origin(method, i.fill_origin);
}

/// An origin the method has no use for maps onto the one it does: `Bottom` and `Left` (the start),
/// `Top` and `Right` (the end).
pub fn set_fill_origin(i: &mut ImageComponent, origin: FillOrigin) {
    i.fill_origin = fit_origin(i.fill_method, origin);
}

pub fn set_fill_amount(i: &mut ImageComponent, amount: f32) {
    i.fill_amount = unit(amount);
}

pub fn set_fill_clockwise(i: &mut ImageComponent, clockwise: bool) {
    i.fill_clockwise = clockwise;
}

pub fn set_preserve_aspect(i: &mut ImageComponent, preserve: bool) {
    i.preserve_aspect = preserve;
}

pub fn set_raycast_target(i: &mut ImageComponent, target: bool) {
    i.raycast_target = target;
}

/// A gradient with fewer than two stops clears it.
// lint: allow(sanitize) `ui_look::sanitize_gradient` runs every field through the helpers
pub fn set_gradient(i: &mut ImageComponent, g: Option<UiGradient>) {
    i.gradient = g.and_then(sanitize_gradient);
}

pub fn set_blend(i: &mut ImageComponent, blend: UiBlend) {
    i.blend = blend;
}

fn fit_origin(method: FillMethod, origin: FillOrigin) -> FillOrigin {
    use FillOrigin::{Bottom, Left, Right, Top};
    match (method, origin) {
        (FillMethod::Horizontal, Bottom) => Left,
        (FillMethod::Horizontal, Top) => Right,
        (FillMethod::Vertical, Left) => Bottom,
        (FillMethod::Vertical, Right) => Top,
        (_, o) => o,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ops_clamp_and_normalize() {
        let mut i = ImageComponent::default();
        set_color(&mut i, Vec4::new(2.0, -1.0, 0.5, 0.25));
        assert_eq!(i.color, Vec4::new(1.0, 0.0, 0.5, 0.25));
        set_border(&mut i, Vec4::new(-3.0, 4.0, 5.0, 6.0));
        assert_eq!(i.border, Vec4::new(0.0, 4.0, 5.0, 6.0));
        set_fill_amount(&mut i, 1.5);
        assert_eq!(i.fill_amount, 1.0);
        set_texture(&mut i, Some(String::new()));
        assert_eq!(i.texture, None);
        set_texture(&mut i, Some("ui/bar.png".into()));
        assert_eq!(i.texture, Some("ui/bar.png".into()));
    }

    #[test]
    fn fill_origin_follows_the_method() {
        let mut i = ImageComponent::default();
        set_fill_origin(&mut i, FillOrigin::Top);
        assert_eq!(i.fill_origin, FillOrigin::Right, "horizontal has no top");
        set_fill_method(&mut i, FillMethod::Vertical);
        assert_eq!(i.fill_origin, FillOrigin::Top, "the end stays the end");
        set_fill_method(&mut i, FillMethod::Radial360);
        set_fill_origin(&mut i, FillOrigin::Left);
        assert_eq!(i.fill_origin, FillOrigin::Left, "radial takes any edge");
    }
}
