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

/// A panorama that fails to decode is decoded once, not on every frame, and a
/// changed sidecar (Max Size) re-projects the same bytes (#1675).
#[test]
fn the_memo_keeps_failures_and_follows_the_sidecar() {
    use super::{decodes, radiance};
    use crate::asset::sidecar::{self, ImportSettings};
    let dir = crate::test_temp::dir();
    let broken = dir.join("broken.png");
    std::fs::write(&broken, b"\x89PNG\r\n\x1a\n truncated").unwrap();
    let broken = broken.to_string_lossy().into_owned();
    let before = decodes();
    assert_eq!(radiance(&broken), None);
    assert_eq!(radiance(&broken), None);
    assert_eq!(decodes(), before + 1, "a failure is remembered");

    let sky = crate::scene::panorama_fixture("sized.png", [200, 0, 0], [0, 0, 200], 0);
    let first = radiance(&sky).expect("decodes");
    assert_eq!(radiance(&sky), Some(first));
    assert_eq!(decodes(), before + 2, "a success is remembered");
    let mut settings = ImportSettings::default();
    settings.texture.max_size = 32;
    sidecar::save(std::path::Path::new(&sky), &settings).unwrap();
    assert!(radiance(&sky).is_some());
    assert_eq!(decodes(), before + 3, "a new Max Size re-projects");
}
