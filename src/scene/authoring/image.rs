//! src/scene/authoring/image.rs — Shared Image-authoring ops (#418).
//!
//! The ONE place the engine knows how to mutate an entity's first-class
//! `ImageComponent` field by field. The editor's Image card and the Lua `Image.*`
//! namespace both route every write through these, so validation lives once: the
//! colour and fill amount stay in `[0, 1]`, borders are non-negative, an empty
//! texture path means "no texture", and the fill origin always suits the fill
//! method (Unity's per-method origin enums, folded into one).
//!
//! Pure.

use glam::Vec4;

use super::sanitize::{finite4, unit, unit_color};
use super::ui_look::sanitize_gradient;
use crate::asset::MapRef;
use crate::components::{FillMethod, FillOrigin, ImageComponent, ImageType, UiBlend, UiGradient};

/// Set the RGBA tint (display space, straight alpha), each channel clamped to `[0, 1]`.
pub fn set_color(i: &mut ImageComponent, color: Vec4) {
    i.color = unit_color(color);
}

/// Set the texture path; `None` or an empty path draws a solid colour.
pub fn set_texture(i: &mut ImageComponent, path: Option<String>) {
    i.texture = path.filter(|p| !p.is_empty()).map(MapRef::from);
}

/// Set how the texture maps onto the rect.
pub fn set_image_type(i: &mut ImageComponent, image_type: ImageType) {
    i.image_type = image_type;
}

/// Set the 9-slice borders in texels (left, bottom, right, top), each kept ≥ 0.
pub fn set_border(i: &mut ImageComponent, border: Vec4) {
    i.border = finite4(border).max(Vec4::ZERO);
}

/// Set the fill shape, re-fitting the origin to it.
pub fn set_fill_method(i: &mut ImageComponent, method: FillMethod) {
    i.fill_method = method;
    i.fill_origin = fit_origin(method, i.fill_origin);
}

/// Set where the fill starts. An origin the method has no use for maps onto the
/// one it does: `Bottom` ↔ `Left` (the start), `Top` ↔ `Right` (the end).
pub fn set_fill_origin(i: &mut ImageComponent, origin: FillOrigin) {
    i.fill_origin = fit_origin(i.fill_method, origin);
}

/// Set the visible fraction of a `Filled` image, clamped to `[0, 1]`.
pub fn set_fill_amount(i: &mut ImageComponent, amount: f32) {
    i.fill_amount = unit(amount);
}

/// Set the radial sweep's direction.
pub fn set_fill_clockwise(i: &mut ImageComponent, clockwise: bool) {
    i.fill_clockwise = clockwise;
}

/// Set whether a `Simple` image keeps its texture's aspect.
pub fn set_preserve_aspect(i: &mut ImageComponent, preserve: bool) {
    i.preserve_aspect = preserve;
}

/// Set whether the pointer can hit this graphic.
pub fn set_raycast_target(i: &mut ImageComponent, target: bool) {
    i.raycast_target = target;
}

/// Set (or clear, `None`) the gradient tint; one with fewer than two stops clears it.
pub fn set_gradient(i: &mut ImageComponent, g: Option<UiGradient>) {
    i.gradient = g.and_then(sanitize_gradient);
}

/// Set how it composites onto the frame.
pub fn set_blend(i: &mut ImageComponent, blend: UiBlend) {
    i.blend = blend;
}

/// The origin `method` uses in place of `origin`.
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
