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
    // NaN in either compares false, so it lands here too.
    if d.partial_cmp(&range) != Some(std::cmp::Ordering::Less) {
        return 0.0;
    }
    let ratio = d / range;
    let r2 = ratio * ratio;
    let window = (1.0 - r2 * r2).clamp(0.0, 1.0);
    window * window / (d * d).max(FALLOFF_EPSILON_SQ)
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
