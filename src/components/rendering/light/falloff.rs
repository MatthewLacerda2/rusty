//! How a point, spot or area light's light fades (#1699): the one model every
//! path that lights by distance uses. The forward shader's copy is
//! `lighting/falloff.wgsl`'s `light_falloff` / `spot_cone`, held equal to these by
//! `render::clusters::falloff_gpu_tests` on a table of distances and angles.
//!
//! It is Unity URP's and Frostbite's windowed inverse-square (Lagarde & de
//! Rousiers, *Moving Frostbite to PBR*, 2014), so a lamp's pool of light reaches
//! exactly zero at its range with no visible edge, and URP's squared spot cone.

/// The squared-distance floor of [`falloff`], URP's: a surface touching a lamp
/// stays finite.
pub const FALLOFF_EPSILON_SQ: f32 = 1e-4;

/// The fraction of a light's intensity that reaches distance `d` from it, for a
/// light of `range`: `saturate(1 − (d/r)⁴)² / max(d², ε²)`. Inverse-square up
/// close, smoothly windowed to exactly zero at `range`; zero at and past it, and
/// for a `range` of zero or less, NaN or a NaN `d`.
pub fn falloff(d: f32, range: f32) -> f32 {
    falloff_near(d, range, 0.0)
}

/// [`falloff`] with its inverse square floored at distance `near` (#1699): for a
/// receiver that stands for more than a point (a particle sprite `near` across),
/// or an emitter that is more than one (an Area light's stand-in), so the light
/// never climbs past what something that size receives. The [`falloff_window`] still runs
/// on `d`, so it reaches zero at `range` whatever `near` is.
pub fn falloff_near(d: f32, range: f32, near: f32) -> f32 {
    falloff_window(d, range) / (d * d).max(near * near).max(FALLOFF_EPSILON_SQ)
}

/// The smooth window of [`falloff`] alone, `saturate(1 − (d/r)⁴)²`: 1 at the lamp,
/// exactly 0 at and past `range`, and 0 for a `range` of zero or less or a NaN.
pub fn falloff_window(d: f32, range: f32) -> f32 {
    // NaN in either compares false, so it lands here too.
    if d.partial_cmp(&range) != Some(std::cmp::Ordering::Less) {
        return 0.0;
    }
    let ratio = d / range;
    let r2 = ratio * ratio;
    let w = (1.0 - r2 * r2).clamp(0.0, 1.0);
    w * w
}

/// [`falloff_window`] at squared distance `d2`, for a caller that never takes the root
/// (the cluster binner's per-pair score).
pub fn falloff_window_sq(d2: f32, range: f32) -> f32 {
    let r2 = range * range;
    if !(range > 0.0 && d2 < r2) {
        return 0.0;
    }
    let t = d2 / r2;
    let w = (1.0 - t * t).clamp(0.0, 1.0);
    w * w
}

/// A spotlight's cone at a direction whose cosine to its axis is `cos_theta`:
/// 1 inside the inner cone, 0 outside the outer one, and between them URP's
/// squared ramp, `saturate((cos θ − cos outer) / (cos inner − cos outer))²`, so
/// the beam's edge has no hard line.
pub fn spot_cone(cos_theta: f32, cos_inner: f32, cos_outer: f32) -> f32 {
    let t = ((cos_theta - cos_outer) / (cos_inner - cos_outer).max(1e-4)).clamp(0.0, 1.0);
    t * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falloff_is_inverse_square_near_and_exactly_zero_at_range() {
        // Far inside the range the window is ~1: plain inverse-square.
        assert!((falloff(1.0, 100.0) - 1.0).abs() < 1e-6);
        assert!((falloff(2.0, 100.0) - 0.25).abs() < 1e-5);
        assert_eq!(falloff(5.0, 5.0), 0.0);
        assert_eq!(falloff(6.0, 5.0), 0.0);
        // Approaching the range it fades to nothing, with no step.
        let edge = falloff(4.99, 5.0);
        assert!(edge > 0.0 && edge < 1e-5, "edge {edge}");
    }

    #[test]
    fn falloff_is_monotonic_and_finite_up_to_the_lamp() {
        assert_eq!(falloff(0.0, 5.0), 1.0 / FALLOFF_EPSILON_SQ);
        let mut last = f32::INFINITY;
        for i in 0..=500 {
            let f = falloff(i as f32 * 0.01, 5.0);
            assert!(f.is_finite() && f <= last, "at {i}");
            last = f;
        }
    }

    #[test]
    fn falloff_of_a_degenerate_light_is_zero() {
        for (d, r) in [(0.0, 0.0), (1.0, -1.0), (f32::NAN, 5.0), (1.0, f32::NAN)] {
            assert_eq!(falloff(d, r), 0.0, "d {d} r {r}");
        }
    }

    #[test]
    fn the_near_floor_caps_the_inverse_square_and_keeps_the_window() {
        assert_eq!(falloff_near(0.0, 5.0, 0.5), 4.0);
        assert_eq!(falloff_near(0.1, 5.0, 0.5), 4.0 * falloff_window(0.1, 5.0));
        // Past `near` it is the plain falloff; at the range it is still zero.
        assert_eq!(falloff_near(2.0, 5.0, 0.5), falloff(2.0, 5.0));
        assert_eq!(falloff_near(5.0, 5.0, 10.0), 0.0);
    }

    #[test]
    fn the_squared_window_is_the_window() {
        for i in 0..=60 {
            let d = i as f32 * 0.1;
            let (a, b) = (falloff_window(d, 5.0), falloff_window_sq(d * d, 5.0));
            assert!((a - b).abs() < 1e-5, "at {d}: {a} vs {b}");
        }
        for (d2, r) in [(0.0, 0.0), (0.25, -1.0), (f32::NAN, 5.0), (1.0, f32::NAN)] {
            assert_eq!(falloff_window_sq(d2, r), 0.0, "d2 {d2} r {r}");
        }
    }

    #[test]
    fn spot_cone_is_a_squared_ramp() {
        let (inner, outer) = (0.9, 0.5);
        assert_eq!(spot_cone(1.0, inner, outer), 1.0);
        assert_eq!(spot_cone(0.4, inner, outer), 0.0);
        assert!((spot_cone(0.7, inner, outer) - 0.25).abs() < 1e-6);
        // Equal cones: a hard edge, never a division by zero.
        assert_eq!(spot_cone(0.8, 0.8, 0.8), 0.0);
        assert_eq!(spot_cone(0.81, 0.8, 0.8), 1.0);
    }
}
