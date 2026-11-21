use glam::Vec3;

use super::{blocks, decodes, project, radiance, ROWS};
use crate::AmbientRadiance;
use rusty_core::fs::written;

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

#[test]
fn the_memo_keeps_failures_and_follows_the_sidecar() {
    use rusty_asset::sidecar::{self, ImportSettings};
    let dir = rusty_test_support::temp::dir();
    let broken = dir.join("broken.png");
    std::fs::write(&broken, b"\x89PNG\r\n\x1a\n truncated").unwrap();
    let broken = broken.to_string_lossy().into_owned();
    let before = decodes();
    assert_eq!(radiance(&broken), None);
    assert_eq!(radiance(&broken), None);
    assert_eq!(decodes(), before + 1, "a failure is remembered");

    let sky = rusty_test_support::panorama::fixture("sized.png", [200, 0, 0], [0, 0, 200], 0);
    let first = radiance(&sky).expect("decodes");
    assert_eq!(radiance(&sky), Some(first));
    assert_eq!(decodes(), before + 2, "a success is remembered");
    let mut settings = ImportSettings::default();
    settings.texture.max_size = 32;
    sidecar::save(std::path::Path::new(&sky), &settings).unwrap();
    assert!(radiance(&sky).is_some());
    assert_eq!(decodes(), before + 3, "a new Max Size re-projects");
}

#[test]
fn the_row_table_weighs_every_texel_as_its_own_cosines_do() {
    use std::f32::consts::PI;
    let (w, h) = (2048, 1024);
    let sky = smooth(w, h);
    let side = h / ROWS;
    let gw = w.div_ceil(side);
    let mut per_texel = vec![(Vec3::ZERO, 0.0f32); (gw * h.div_ceil(side)) as usize];
    let row = |y: u32| {
        let band = |y: u32| (y as f32 / h as f32 * PI).cos();
        (band(y) - band(y + 1)) * 2.0 * PI / w as f32
    };
    for (x, y, p) in sky.enumerate_pixels() {
        let rgb = Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32) / 255.0;
        let block = &mut per_texel[((y / side) * gw + x / side) as usize];
        block.0 += rgb * row(y);
        block.1 += row(y);
    }
    let bits = |b: &[(Vec3, f32)]| -> Vec<[u32; 4]> {
        let bits = |(c, s): &(Vec3, f32)| [c.x, c.y, c.z, *s].map(f32::to_bits);
        b.iter().map(bits).collect()
    };
    assert_eq!(bits(&blocks(w, h, sky.as_raw(), side)), bits(&per_texel));
}

fn red(sh: Option<crate::Sh9>) -> Option<f32> {
    sh.map(|sh| sh.coeffs[0][0] - sh.coeffs[0][2])
}

#[test]
fn a_remembered_panorama_reads_no_file_until_a_write_is_noted() {
    let sky = rusty_test_support::panorama::fixture("kept.png", [200, 0, 0], [200, 0, 0], 0);
    let meta = format!("{sky}.meta");
    std::fs::write(&meta, "{}").unwrap();
    let first = radiance(&sky).expect("decodes");
    let before = decodes();
    std::fs::remove_file(&meta).unwrap();
    std::fs::remove_file(&sky).unwrap();
    for _ in 0..3 {
        assert_eq!(radiance(&sky), Some(first), "no file read, no decode");
    }
    assert_eq!(decodes(), before);
    written::note_written(&meta);
    assert_eq!(radiance(&sky), None, "a noted sidecar re-reads the file");
    let blue = rusty_test_support::panorama::fixture("kept.png", [0, 0, 200], [0, 0, 200], 0);
    assert_eq!(
        radiance(&blue),
        None,
        "a miss waits for a write or its retry"
    );
    written::note_written(&blue);
    assert!(
        red(radiance(&blue)).unwrap() < 0.0,
        "a noted panorama re-projects"
    );
}

#[test]
fn the_panorama_memo_follows_rebakes() {
    use rusty_test_support::rebakes::{follows_rebakes, Bake};
    let colour = |b: Bake| {
        if b == Bake::A {
            [200, 0, 0]
        } else {
            [0, 0, 200]
        }
    };
    let source = |b: Bake| {
        let name = format!("source_{b:?}.png");
        let c = colour(b);
        std::fs::read(rusty_test_support::panorama::fixture(&name, c, c, 0)).unwrap()
    };
    let path = rusty_test_support::temp::dir().join("rebaked.png");
    follows_rebakes(
        &path.to_string_lossy(),
        |p, b| written::write(std::path::Path::new(p), &source(b)).unwrap(),
        |p| red(radiance(p)),
        |r, b| r.is_some_and(|r| (r > 0.0) == (b == Bake::A)),
    );
}
