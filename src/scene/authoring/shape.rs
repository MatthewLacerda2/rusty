//! src/scene/authoring/shape.rs — Shared Shape-authoring ops (#425).
//!
//! The ONE place the engine mutates an entity's first-class `ShapeComponent`. The
//! editor's Shape card and the Lua `Shape.*` namespace both route every write
//! through these, so validation lives once: colours stay in `[0, 1]`, lengths are
//! non-negative, and the gradient is sanitized by the shared `ui_look` op.
//!
//! Pure.

use glam::{Vec2, Vec4};

use super::ui_look::sanitize_gradient;
use crate::components::{
    ShapeComponent, ShapeCorner, ShapeGlow, ShapeKind, ShapeShadow, UiBlend, UiGradient,
};

/// Set which primitive it draws.
pub fn set_kind(s: &mut ShapeComponent, kind: ShapeKind) {
    s.kind = kind;
}

/// Set whether a `Rect`'s corners are rounded or chamfered.
pub fn set_corner(s: &mut ShapeComponent, corner: ShapeCorner) {
    s.corner = corner;
}

/// Set the per-corner radius (top-left, top-right, bottom-right, bottom-left), each ≥ 0.
pub fn set_radius(s: &mut ShapeComponent, radius: Vec4) {
    s.radius = radius.max(Vec4::ZERO);
}

/// Set a `Ring`'s hole radius (≥ 0).
pub fn set_inner_radius(s: &mut ShapeComponent, r: f32) {
    s.inner_radius = r.max(0.0);
}

/// Set a `Ring`'s arc, degrees clockwise from 12 o'clock.
pub fn set_arc(s: &mut ShapeComponent, start: f32, end: f32) {
    (s.arc_start, s.arc_end) = (start, end);
}

/// Set a `Line`'s thickness (≥ 0).
pub fn set_thickness(s: &mut ShapeComponent, t: f32) {
    s.thickness = t.max(0.0);
}

/// Set a `Line`'s dash and gap lengths (each ≥ 0; a dash of 0 is solid).
pub fn set_dash(s: &mut ShapeComponent, dash: f32, gap: f32) {
    (s.dash, s.gap) = (dash.max(0.0), gap.max(0.0));
}

/// Set the fill colour, clamped to `[0, 1]`.
pub fn set_color(s: &mut ShapeComponent, color: Vec4) {
    s.color = unit(color);
}

/// Set (or clear, `None`) the gradient fill; one with fewer than two stops clears it.
pub fn set_gradient(s: &mut ShapeComponent, g: Option<UiGradient>) {
    s.gradient = g.and_then(sanitize_gradient);
}

/// Set the border band's width (≥ 0) and colour.
pub fn set_border(s: &mut ShapeComponent, width: f32, color: Vec4) {
    (s.border_width, s.border_color) = (width.max(0.0), unit(color));
}

/// Set the drop shadow (blur ≥ 0; a transparent colour turns it off).
pub fn set_shadow(s: &mut ShapeComponent, offset: Vec2, blur: f32, color: Vec4) {
    s.shadow = ShapeShadow {
        offset,
        blur: blur.max(0.0),
        color: unit(color),
    };
}

/// Set the outer glow (size and intensity ≥ 0; 0 or a transparent colour is off).
pub fn set_glow(s: &mut ShapeComponent, size: f32, intensity: f32, color: Vec4) {
    s.glow = ShapeGlow {
        size: size.max(0.0),
        intensity: intensity.max(0.0),
        color: unit(color),
    };
}

/// Set how it composites onto the frame.
pub fn set_blend(s: &mut ShapeComponent, blend: UiBlend) {
    s.blend = blend;
}

/// Set whether the pointer can hit it.
pub fn set_raycast_target(s: &mut ShapeComponent, target: bool) {
    s.raycast_target = target;
}

/// `c` clamped to `[0, 1]`.
fn unit(c: Vec4) -> Vec4 {
    c.clamp(Vec4::ZERO, Vec4::ONE)
}

/// The kind's name.
pub fn kind_name(k: ShapeKind) -> &'static str {
    match k {
        ShapeKind::Rect => "Rect",
        ShapeKind::Ellipse => "Ellipse",
        ShapeKind::Ring => "Ring",
        ShapeKind::Line => "Line",
    }
}

/// Parse a kind name (case-insensitive).
pub fn parse_kind(name: &str) -> Option<ShapeKind> {
    use ShapeKind::{Ellipse, Line, Rect, Ring};
    [Rect, Ellipse, Ring, Line]
        .into_iter()
        .find(|k| kind_name(*k).eq_ignore_ascii_case(name))
}

/// The corner style's name.
pub fn corner_name(c: ShapeCorner) -> &'static str {
    match c {
        ShapeCorner::Round => "Round",
        ShapeCorner::Chamfer => "Chamfer",
    }
}

/// Parse a corner-style name (case-insensitive).
pub fn parse_corner(name: &str) -> Option<ShapeCorner> {
    [ShapeCorner::Round, ShapeCorner::Chamfer]
        .into_iter()
        .find(|c| corner_name(*c).eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_stay_non_negative_and_colours_in_range() {
        let mut s = ShapeComponent::default();
        set_radius(&mut s, Vec4::new(-1.0, 2.0, 3.0, 4.0));
        set_dash(&mut s, -3.0, 2.0);
        set_border(&mut s, -1.0, Vec4::splat(2.0));
        set_glow(&mut s, 8.0, -1.0, Vec4::ONE);
        assert_eq!(s.radius, Vec4::new(0.0, 2.0, 3.0, 4.0));
        assert_eq!((s.dash, s.gap), (0.0, 2.0));
        assert_eq!((s.border_width, s.border_color), (0.0, Vec4::ONE));
        assert!(!s.glow.is_on(), "zero intensity is off");
    }

    #[test]
    fn names_round_trip() {
        for k in [
            ShapeKind::Rect,
            ShapeKind::Ellipse,
            ShapeKind::Ring,
            ShapeKind::Line,
        ] {
            assert_eq!(parse_kind(&kind_name(k).to_lowercase()), Some(k));
        }
        assert_eq!(parse_corner("CHAMFER"), Some(ShapeCorner::Chamfer));
        assert_eq!(parse_kind("Star"), None);
    }
}
