//! src/scene/authoring/ui/look.rs — Shared UI-look ops: blend and gradient (#425).
//!
//! The ONE place the blend mode and gradient every UI graphic shares are named,
//! parsed and validated. The Image, Text and Shape ops (and so their inspector
//! cards and Lua namespaces) route through these: a gradient keeps 2–4 stops in
//! ascending `t` within `[0, 1]`, colours in `[0, 1]`, and a positive radius.
//!
//! Pure.

use glam::Vec2;

use crate::components::{UiGradient, MAX_GRADIENT_STOPS};
use crate::scene::authoring::sanitize::{finite, unit_color, MIN_EXTENT};

/// `g` made valid, or `None` when it has fewer than two stops: at most
/// [`MAX_GRADIENT_STOPS`] stops (the first ones), sorted by `t`, `t` and colours
/// clamped to `[0, 1]`, the radius kept positive and the angle finite.
pub fn sanitize_gradient(mut g: UiGradient) -> Option<UiGradient> {
    g.stops.truncate(MAX_GRADIENT_STOPS);
    if g.stops.len() < 2 {
        return None;
    }
    for s in &mut g.stops {
        s.t = finite(s.t).clamp(0.0, 1.0);
        s.color = unit_color(s.color);
    }
    g.stops.sort_by(|a, b| a.t.total_cmp(&b.t));
    g.angle = finite(g.angle);
    g.center = Vec2::new(finite(g.center.x), finite(g.center.y));
    g.radius = finite(g.radius).max(MIN_EXTENT);
    Some(g)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::GradientStop;
    use glam::Vec4;

    fn stop(t: f32) -> GradientStop {
        GradientStop {
            t,
            color: Vec4::splat(2.0),
        }
    }

    #[test]
    fn a_gradient_is_sorted_clamped_capped_and_needs_two_stops() {
        let g = UiGradient {
            stops: vec![stop(0.9), stop(-1.0), stop(0.5), stop(0.2), stop(0.1)],
            radius: -1.0,
            ..Default::default()
        };
        let g = sanitize_gradient(g).expect("valid");
        let ts: Vec<f32> = g.stops.iter().map(|s| s.t).collect();
        assert_eq!(ts, vec![0.0, 0.2, 0.5, 0.9]);
        assert_eq!(g.stops[0].color, Vec4::ONE);
        assert!(g.radius > 0.0);
        let one = UiGradient {
            stops: vec![stop(0.0)],
            ..Default::default()
        };
        assert_eq!(sanitize_gradient(one), None);
    }
}
