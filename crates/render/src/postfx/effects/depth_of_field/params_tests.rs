//! The depth of field's params and circle of confusion (#1445), without an adapter.

use super::*;

fn dof(mode: DofMode) -> DepthOfField {
    DepthOfField {
        mode,
        ..DepthOfField::default()
    }
}

#[test]
fn off_draws_nothing() {
    assert!(DofParams::new(&dof(DofMode::Off), 1080, 2).is_none());
}

#[test]
fn gaussian_ramps_the_background_from_start_to_end() {
    let p = DofParams::new(&dof(DofMode::Gaussian), 1080, 2).unwrap();
    assert_eq!(p.divisor(), 2);
    let full = 1080.0 * GAUSSIAN_RADIUS;
    assert_eq!(p.coc(2.0), 0.0, "nearer than the start is sharp");
    assert_eq!(p.coc(10.0), 0.0);
    assert!(
        (p.coc(20.0) - full * 0.5).abs() < 1e-3,
        "half way, half the radius"
    );
    assert!(
        (p.coc(500.0) - full).abs() < 1e-3,
        "past the end, all of it"
    );
}

#[test]
fn bokeh_blurs_both_sides_of_the_focus_and_grows_with_the_lens() {
    let mut lens = dof(DofMode::Bokeh);
    lens.focus_distance = 2.0;
    let p = DofParams::new(&lens, 1080, 1).unwrap();
    assert!(p.coc(2.0).abs() < 1e-4, "the focus is sharp");
    assert!(
        p.coc(1.0) < 0.0 && p.coc(8.0) > 0.0,
        "near negative, far positive"
    );
    // A 50 mm lens at f/5.6 focused at 2 m: f² / (N (F − f)) on a 24 mm sensor.
    let infinity = 0.5 * 0.05 * 0.05 / (5.6 * 1.95) / SENSOR_HEIGHT * 1080.0;
    assert!(
        (p.coc(1e9) - infinity).abs() < 1e-2,
        "{} vs {infinity}",
        p.coc(1e9)
    );
    lens.aperture = 1.4;
    lens.focal_length = 85.0;
    let wide = DofParams::new(&lens, 1080, 1).unwrap();
    let max = 1080.0 * MAX_BOKEH_RADIUS;
    assert!(
        wide.coc(30.0) > p.coc(30.0),
        "a faster, longer lens blurs more"
    );
    assert_eq!(wide.coc(1e9), max, "clamped to the largest circle");
    assert_eq!(wide.coc(0.2), -max);
}

#[test]
fn bokeh_tiles_span_the_largest_working_blur_and_gaussian_has_none() {
    // 27 px at 1080p, 13.5 working px at half resolution: 14 px tiles, so a 3×3 of
    // them holds every tap's reach.
    let half = DofParams::new(&dof(DofMode::Bokeh), 1080, 2).unwrap();
    assert_eq!(half.tile(), 14);
    let full = DofParams::new(&dof(DofMode::Bokeh), 1080, 1).unwrap();
    assert_eq!(full.tile(), 27);
    let gaussian = DofParams::new(&dof(DofMode::Gaussian), 1080, 1).unwrap();
    assert_eq!(gaussian.tile(), 0);
}

#[test]
fn bokeh_rings_stay_close_at_every_resolution_and_the_circle_caps_past_4k() {
    for (height, divisor) in [(1080, 1), (1440, 1), (2160, 1), (2160, 2), (4320, 1)] {
        let p = DofParams::new(&dof(DofMode::Bokeh), height, divisor).unwrap();
        let (max, spacing) = (p.shape[1] / divisor as f32, p.focus[2]);
        // The shader's ring count: a ring per spacing, at least three.
        let rings = (max / spacing).ceil().max(3.0);
        assert!(rings <= BOKEH_MAX_RINGS, "{height}p: {rings} rings");
        assert!(
            max / rings <= spacing,
            "{height}p: rings {} apart",
            max / rings
        );
    }
    assert_eq!(max_bokeh_radius(2160.0), 54.0, "4K keeps its whole circle");
    assert_eq!(max_bokeh_radius(4320.0), 56.0, "taller views cap");
}
