//! URP's windowed inverse-square (Lagarde & de Rousiers, 2014) and squared spot cone; held equal
//! to `lighting/falloff.wgsl` by `render::clusters::falloff_gpu_tests`.

/// URP's squared-distance floor: a surface touching a lamp stays finite.
pub const FALLOFF_EPSILON_SQ: f32 = 1e-4;

/// `saturate(1 − (d/r)⁴)² / max(d², ε²)`; zero at and past `range`, and for NaN or `range` ≤ 0.
pub fn falloff(d: f32, range: f32) -> f32 {
    falloff_near(d, range, 0.0)
}

/// [`falloff`] with the inverse square floored at distance `near`; the window still runs on `d`.
pub fn falloff_near(d: f32, range: f32, near: f32) -> f32 {
    falloff_window(d, range) / (d * d).max(near * near).max(FALLOFF_EPSILON_SQ)
}

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

/// For a caller that never takes the root: the cluster binner.
pub fn falloff_window_sq(d2: f32, range: f32) -> f32 {
    let r2 = range * range;
    if !(range > 0.0 && d2 < r2) {
        return 0.0;
    }
    let t = d2 / r2;
    let w = (1.0 - t * t).clamp(0.0, 1.0);
    w * w
}

/// URP's squared ramp, `saturate((cos θ − cos outer) / (cos inner − cos outer))²`.
pub fn spot_cone(cos_theta: f32, cos_inner: f32, cos_outer: f32) -> f32 {
    let t = ((cos_theta - cos_outer) / (cos_inner - cos_outer).max(1e-4)).clamp(0.0, 1.0);
    t * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falloff_is_inverse_square_near_and_exactly_zero_at_range() {
        assert!((falloff(1.0, 100.0) - 1.0).abs() < 1e-6);
        assert!((falloff(2.0, 100.0) - 0.25).abs() < 1e-5);
        assert_eq!(falloff(5.0, 5.0), 0.0);
        assert_eq!(falloff(6.0, 5.0), 0.0);
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
