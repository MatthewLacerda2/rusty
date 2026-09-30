//! src/scene/authoring/image.rs — Shared Image-authoring ops (#418).
//!
//! The ONE place the engine knows how to mutate an entity's first-class
//! `ImageComponent` field by field. The editor's Image card and the Lua `Image.*`
//! namespace both route every write through these, so validation lives once: the
//! colour and fill amount stay in `[0, 1]`, borders are non-negative, an empty
//! texture path means "no texture", and the fill origin always suits the fill
//! method (Unity's per-method origin enums, folded into one).
//!
//! Allowed deps: components (the `ImageComponent` data). Pure.

use glam::Vec4;

use crate::components::{FillMethod, FillOrigin, ImageComponent, ImageType};

/// Set the RGBA tint (display space, straight alpha), each channel clamped to `[0, 1]`.
pub fn set_color(i: &mut ImageComponent, color: Vec4) {
    i.color = color.clamp(Vec4::ZERO, Vec4::ONE);
}

/// Set the texture path; `None` or an empty path draws a solid colour.
pub fn set_texture(i: &mut ImageComponent, path: Option<String>) {
    i.texture = path.filter(|p| !p.is_empty());
}

/// Set how the texture maps onto the rect.
pub fn set_image_type(i: &mut ImageComponent, image_type: ImageType) {
    i.image_type = image_type;
}

/// Set the 9-slice borders in texels (left, bottom, right, top), each kept ≥ 0.
pub fn set_border(i: &mut ImageComponent, border: Vec4) {
    i.border = border.max(Vec4::ZERO);
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
    i.fill_amount = amount.clamp(0.0, 1.0);
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

/// Parse an image-type name (case-insensitive).
pub fn parse_image_type(name: &str) -> Option<ImageType> {
    [
        ImageType::Simple,
        ImageType::Sliced,
        ImageType::Tiled,
        ImageType::Filled,
    ]
    .into_iter()
    .find(|t| image_type_name(*t).eq_ignore_ascii_case(name))
}

/// The image type's Unity name.
pub fn image_type_name(t: ImageType) -> &'static str {
    match t {
        ImageType::Simple => "Simple",
        ImageType::Sliced => "Sliced",
        ImageType::Tiled => "Tiled",
        ImageType::Filled => "Filled",
    }
}

/// Parse a fill-method name (case-insensitive).
pub fn parse_fill_method(name: &str) -> Option<FillMethod> {
    [
        FillMethod::Horizontal,
        FillMethod::Vertical,
        FillMethod::Radial360,
    ]
    .into_iter()
    .find(|m| fill_method_name(*m).eq_ignore_ascii_case(name))
}

/// The fill method's Unity name.
pub fn fill_method_name(m: FillMethod) -> &'static str {
    match m {
        FillMethod::Horizontal => "Horizontal",
        FillMethod::Vertical => "Vertical",
        FillMethod::Radial360 => "Radial360",
    }
}

/// Parse a fill-origin name (case-insensitive).
pub fn parse_fill_origin(name: &str) -> Option<FillOrigin> {
    use FillOrigin::{Bottom, Left, Right, Top};
    [Left, Right, Bottom, Top]
        .into_iter()
        .find(|o| fill_origin_name(*o).eq_ignore_ascii_case(name))
}

/// The fill origin's name.
pub fn fill_origin_name(o: FillOrigin) -> &'static str {
    match o {
        FillOrigin::Left => "Left",
        FillOrigin::Right => "Right",
        FillOrigin::Bottom => "Bottom",
        FillOrigin::Top => "Top",
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
        assert_eq!(i.texture.as_deref(), Some("ui/bar.png"));
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

    #[test]
    fn names_round_trip() {
        for t in [
            ImageType::Simple,
            ImageType::Sliced,
            ImageType::Tiled,
            ImageType::Filled,
        ] {
            assert_eq!(parse_image_type(image_type_name(t)), Some(t));
        }
        assert_eq!(parse_fill_method("radial360"), Some(FillMethod::Radial360));
        assert_eq!(parse_fill_origin("TOP"), Some(FillOrigin::Top));
        assert_eq!(parse_image_type("Mesh"), None);
    }
}
