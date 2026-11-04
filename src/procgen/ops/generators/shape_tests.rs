use super::*;
use crate::procgen::image_buf::Rgba;

fn op(kind: ShapeKind, size: [f32; 2], softness: f32) -> Shape {
    Shape {
        kind,
        size,
        roundness: 0.5,
        softness: softness.into(),
        sides: 6,
        points: 5,
        inner_radius: 0.5,
        profile: ShapeProfile::Flat,
    }
}

fn at(kind: ShapeKind, size: [f32; 2], softness: f32, u: f32, v: f32) -> f32 {
    sampler(&op(kind, size, softness), Field::Const(softness))(u, v)[0]
}

#[test]
fn each_kind_covers_its_centre_and_not_its_outside() {
    use ShapeKind::*;
    for kind in [Circle, Rect, RoundedRect, Line, Polygon, Star] {
        assert_eq!(at(kind, [0.6, 0.2], 0.0, 0.5, 0.5), 1.0, "{kind:?}");
        assert_eq!(at(kind, [0.6, 0.2], 0.0, 0.5, 0.75), 0.0, "{kind:?}");
        assert_eq!(at(kind, [0.6, 0.2], 0.0, 0.85, 0.5), 0.0, "{kind:?}");
    }
}

#[test]
fn corners_tell_rect_rounded_rect_and_circle_apart() {
    // Just inside the corner of a 0.6-square.
    let (u, v) = (0.79, 0.79);
    assert_eq!(at(ShapeKind::Rect, [0.6, 0.6], 0.0, u, v), 1.0);
    assert_eq!(at(ShapeKind::RoundedRect, [0.6, 0.6], 0.0, u, v), 0.0);
    assert_eq!(at(ShapeKind::Circle, [0.6, 0.6], 0.0, u, v), 0.0);
    assert_eq!(at(ShapeKind::Circle, [0.6, 0.6], 0.0, 0.79, 0.5), 1.0);
}

#[test]
fn softness_ramps_inward_from_the_edge() {
    // Circle of radius 0.3: 0.05 inside the edge, with a 0.1 ramp, reads 0.5.
    let mid = at(ShapeKind::Circle, [0.6, 0.6], 0.1, 0.75, 0.5);
    assert!((mid - 0.5).abs() < 1e-4, "{mid}");
    assert_eq!(at(ShapeKind::Circle, [0.6, 0.6], 0.1, 0.5, 0.5), 1.0);
    assert_eq!(at(ShapeKind::Circle, [0.6, 0.6], 0.1, 0.81, 0.5), 0.0);
}

#[test]
fn a_polygon_has_its_vertex_up_and_its_edge_down() {
    // A triangle of circumradius 0.4: its top vertex at v = 0.1, its bottom edge at
    // the apothem, 0.2 below the centre (v = 0.7).
    let mut tri = op(ShapeKind::Polygon, [0.8, 0.8], 0.0);
    tri.sides = 3;
    let f = sampler(&tri, Field::Const(0.0));
    assert_eq!(f(0.5, 0.12)[0], 1.0, "just under the top vertex");
    assert_eq!(f(0.5, 0.68)[0], 1.0, "just above the bottom edge");
    assert_eq!(f(0.5, 0.72)[0], 0.0, "just below it");
    assert_eq!(f(0.4, 0.12)[0], 0.0, "beside the vertex");
}

#[test]
fn a_star_is_hollow_between_its_points() {
    // Five points of radius 0.4, inner radius 0.2: a point straight up, and the gap
    // between two points (36° off it) dips to 0.2 from the centre.
    let star = op(ShapeKind::Star, [0.8, 0.8], 0.0);
    let f = sampler(&star, Field::Const(0.0));
    let along = |deg: f32, r: f32| {
        let a = deg.to_radians() - FRAC_PI_2;
        f(0.5 + r * a.cos(), 0.5 + r * a.sin())[0]
    };
    assert_eq!(along(0.0, 0.38), 1.0, "inside a point");
    assert_eq!(along(36.0, 0.18), 1.0, "inside the inner vertex");
    assert_eq!(along(36.0, 0.25), 0.0, "past it, between the points");
}

#[test]
fn profiles_read_the_circles_textbook_heights() {
    // A circle of radius 0.4, read halfway out (r/R = 0.5).
    for (profile, want) in [
        (ShapeProfile::Cone, 0.5),
        (ShapeProfile::Paraboloid, 0.75),
        (ShapeProfile::Hemisphere, 0.75f32.sqrt()),
    ] {
        let mut c = op(ShapeKind::Circle, [0.8, 0.8], 0.0);
        c.profile = profile;
        let f = sampler(&c, Field::Const(0.0));
        assert!((f(0.7, 0.5)[0] - want).abs() < 1e-4, "{profile:?}");
        assert!(
            (f(0.5, 0.5)[0] - 1.0).abs() < 1e-6,
            "{profile:?} peaks at 1"
        );
        assert_eq!(f(0.95, 0.5)[0], 0.0, "{profile:?} is 0 outside");
    }
}

#[test]
fn a_spilling_odd_star_meets_itself_at_the_wrap() {
    let mut big = op(ShapeKind::Star, [1.4, 1.4], 0.0);
    (big.points, big.profile) = (3, ShapeProfile::Cone);
    let f = sampler(&big, Field::Const(0.0));
    let near = |a: Rgba, b: Rgba| (a[0] - b[0]).abs() < 1e-3;
    for i in 0..64 {
        let t = i as f32 / 64.0;
        assert!(near(f(t, 0.0), f(t, 1.0 - 1e-5)), "top/bottom at u = {t}");
        assert!(near(f(0.0, t), f(1.0 - 1e-5, t)), "left/right at v = {t}");
    }
}

#[test]
fn malformed_counts_and_radii_are_bounded() {
    for (sides, inner) in [(0, f32::NAN), (u32::MAX, f32::INFINITY), (1, -5.0)] {
        for kind in [ShapeKind::Polygon, ShapeKind::Star] {
            let mut s = op(kind, [0.6, f32::NAN], 0.0);
            (s.sides, s.points, s.inner_radius) = (sides, sides, inner);
            s.profile = ShapeProfile::Hemisphere;
            let x = sampler(&s, Field::Const(0.0))(0.4, 0.6)[0];
            assert!((0.0..=1.0).contains(&x), "{kind:?} {sides} {inner}: {x}");
        }
    }
}
