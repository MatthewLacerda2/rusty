//! The contact fix keeps enough bias (#665): lit surfaces never shadow themselves.
//! With no ambient a self-shadowed texel reads near 0, so acne shows as a patch of
//! one surface far darker than the rest of it. Skips with no adapter.

use glam::Vec3;

use super::{brightness, looking, yard};
use crate::scene::authoring::Primitive;

/// A 5 x 5 grid of points around `centre`, spanning `u` and `v` either side.
fn grid(centre: Vec3, u: Vec3, v: Vec3) -> Vec<Vec3> {
    let steps = [-1.0, -0.5, 0.0, 0.5, 1.0];
    steps
        .iter()
        .flat_map(|a| steps.iter().map(move |b| centre + u * *a + v * *b))
        .collect()
}

/// `(darkest, brightest)` of `points` as seen from `eye`.
fn spread(
    r: &mut crate::render::Renderer,
    scene: &crate::scene::Scene,
    eye: Vec3,
    points: &[Vec3],
) -> (u32, u32) {
    let centre = points[points.len() / 2];
    let b = brightness(r, scene, &looking(eye, centre), points);
    (*b.iter().min().unwrap(), *b.iter().max().unwrap())
}

#[test]
fn gpu_lit_surfaces_cast_no_acne_from_grazing_to_overhead_sun() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(super::RES, super::RES) else {
        return;
    };
    let floor_patch = grid(Vec3::new(-3.0, 0.0, 3.0), Vec3::X * 0.6, Vec3::Z * 0.6);
    let top = grid(Vec3::Y, Vec3::X * 0.3, Vec3::Z * 0.3);
    let sunny_side = grid(Vec3::new(-0.5, 0.5, 0.0), Vec3::Y * 0.3, Vec3::Z * 0.3);
    for floor in [Primitive::Plane, Primitive::Box] {
        for elevation in [15.0, 30.0, 45.0, 60.0, 80.0, 90.0] {
            let scene = yard(elevation, floor, true);
            let mut views = vec![
                ("floor near", Vec3::new(-3.5, 2.5, 4.5), &floor_patch),
                ("floor far", Vec3::new(-3.0, 18.0, 28.0), &floor_patch),
                ("box top", Vec3::new(-1.5, 2.5, 1.0), &top),
            ];
            if elevation < 80.0 {
                views.push(("box sunny side", Vec3::new(-2.5, 0.8, 0.6), &sunny_side));
            }
            for (what, eye, points) in views {
                let (lo, hi) = spread(&mut r, &scene, eye, points);
                eprintln!("[acne] {floor:?} {elevation}° {what}: {lo}..{hi}");
                assert!(hi > 30, "control: the {what} is lit");
                assert!(
                    lo * 10 >= hi * 8,
                    "{floor:?} {what} at {elevation}° has acne: {lo}..{hi}"
                );
            }
        }
    }
}
