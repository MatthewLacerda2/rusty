//! A panorama past `ROWS` rows is summed in blocks (#1675), and lights as the same
//! sky at `ROWS` rows projected texel by texel.

use glam::Vec3;

use super::project;
use crate::scene::AmbientRadiance;

/// A smooth sky, red at the top fading toward the bottom, warmer to the east (+x).
fn smooth(w: u32, h: u32) -> image::RgbaImage {
    image::RgbaImage::from_fn(w, h, |x, y| {
        let east = (std::f32::consts::TAU * x as f32 / w as f32).cos() * -40.0;
        let red = 200.0 * (1.0 - y as f32 / h as f32) + 40.0 + east;
        image::Rgba([red as u8, 0, 0, 255])
    })
}

#[test]
fn a_large_panorama_projects_in_blocks_as_a_small_one_does_texel_by_texel() {
    let (big, small) = (smooth(1024, 512), smooth(256, 128));
    let blocks = AmbientRadiance::Sky(project(1024, 512, big.as_raw()));
    let fine = AmbientRadiance::Sky(project(256, 128, small.as_raw()));
    for n in [Vec3::Y, Vec3::NEG_Y, Vec3::X, Vec3::NEG_X, Vec3::Z] {
        let (b, f) = (blocks.at(n), fine.at(n));
        assert!(b.abs_diff_eq(f, 5e-3), "{n}: {b} vs {f}");
    }
    assert!(blocks.at(Vec3::Y).x > blocks.at(Vec3::NEG_Y).x + 0.1);
    assert!(blocks.at(Vec3::X).x > blocks.at(Vec3::NEG_X).x + 0.02);
}
