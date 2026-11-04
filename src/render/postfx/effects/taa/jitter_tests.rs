use glam::{Mat4, Vec2, Vec3, Vec4};

use super::{halton, jittered, ndc, pixels, SEQUENCE};
use crate::components::Projection;
use crate::render::PixelSize;
use crate::scene::Camera;

#[test]
fn halton_starts_as_urp_does() {
    assert_eq!(
        [halton(1, 2), halton(2, 2), halton(3, 2)],
        [0.5, 0.25, 0.75]
    );
    let third = [halton(1, 3), halton(2, 3), halton(3, 3)];
    assert!((third[0] - 1.0 / 3.0).abs() < 1e-6 && (third[1] - 2.0 / 3.0).abs() < 1e-6);
    assert!((third[2] - 1.0 / 9.0).abs() < 1e-6);
}

#[test]
fn offsets_stay_in_the_pixel_average_to_its_centre_and_repeat() {
    let all: Vec<Vec2> = (0..SEQUENCE).map(pixels).collect();
    assert!(all.iter().all(|p| p.abs().max_element() <= 0.5));
    let mean = all.iter().copied().sum::<Vec2>() / SEQUENCE as f32;
    assert!(mean.abs().max_element() < 0.01, "centred: {mean}");
    assert_eq!(pixels(3), pixels(3 + SEQUENCE));
    assert_ne!(pixels(0), pixels(1));
}

#[test]
fn the_jitter_moves_every_point_by_the_same_fraction_of_a_pixel() {
    let size = PixelSize {
        width: 64,
        height: 48,
    };
    let offset = ndc(Vec2::new(0.5, -0.25), size);
    assert_eq!(offset, Vec2::new(1.0 / 64.0, -1.0 / 96.0));
    let persp = Camera::new(Vec3::ZERO, -90.0, 0.0).build_view_projection(4.0 / 3.0);
    let mut lens = Camera::new(Vec3::ZERO, -90.0, 0.0);
    lens.projection = Projection::Orthographic { size: 3.0 };
    let ortho = lens.build_view_projection(4.0 / 3.0);
    for vp in [persp, ortho] {
        let moved = jittered(vp, offset);
        for p in [Vec3::new(0.3, -0.2, -2.0), Vec3::new(-1.0, 0.5, -30.0)] {
            let ndc_of = |m: Mat4| {
                let c: Vec4 = m * p.extend(1.0);
                (c.truncate() / c.w).truncate()
            };
            let d = ndc_of(moved) - ndc_of(vp);
            assert!((d - offset).abs().max_element() < 1e-5, "{d} vs {offset}");
        }
    }
    assert_eq!(
        jittered(persp, Vec2::ZERO),
        persp,
        "no jitter, the matrix itself"
    );
}
