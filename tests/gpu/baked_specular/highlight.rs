//! The baked lobe against the realtime truth, and a rough plane it leaves alone.

use glam::Vec3;
use rusty::components::LightMode;
use rusty::dev::screenshot::capture;
use rusty::scene::{Camera, Scene};

use super::{bake, scene};

const SIZE: u32 = 96;

/// `scene` from a camera 2.5 m off along -X, 1.5 m up, looking at the lamp's foot:
/// the lamp's mirror image sits 1 m short of it. `None` without an adapter.
fn shot(scene: &Scene, name: &str) -> Option<image::RgbImage> {
    let camera = Camera::new(Vec3::new(-2.5, 1.5, 0.0), 0.0, -31.0);
    let path = crate::temp::dir().join(format!("rusty_gpu_baked_spec_{name}.png"));
    capture(scene, &camera, &path, SIZE, SIZE)
        .expect("capture")
        .then_some(())?;
    Some(image::open(&path).unwrap().to_rgb8())
}

/// The luminance of every pixel, 0..=255.
fn luma(img: &image::RgbImage) -> Vec<f32> {
    img.pixels()
        .map(|p| 0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32)
        .collect()
}

/// Where `a` exceeds `b` most, as (x, y, by how much).
fn peak(a: &[f32], b: &[f32]) -> (u32, u32, f32) {
    let (i, d) =
        a.iter()
            .zip(b)
            .map(|(a, b)| a - b)
            .enumerate()
            .fold(
                (0, f32::MIN),
                |best, (i, d)| if d > best.1 { (i, d) } else { best },
            );
    (i as u32 % SIZE, i as u32 / SIZE, d)
}

/// A glossy plane under the lamp, `Baked` with a `directional` bake, or `Realtime`.
fn glossy(mode: LightMode, directional: bool) -> Option<Vec<f32>> {
    let mut scene = scene(mode, 0.25);
    bake(&mut scene, &format!("{mode:?}_{directional}"), directional);
    shot(&scene, &format!("glossy_{mode:?}_{directional}")).map(|img| luma(&img))
}

/// The highlight's energy: the summed excess of `a` over `b` where it exceeds 2.
fn energy(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(a, b)| a - b)
        .filter(|d| *d > 2.0)
        .sum()
}

#[test]
fn a_baked_lamps_highlight_sits_where_its_realtime_one_does() {
    let Some(baked) = glossy(LightMode::Baked, true) else {
        eprintln!("[baked-spec] no GPU/software adapter — skipping");
        return;
    };
    // The same bake without directions: the same diffuse, no lobe.
    let diffuse = glossy(LightMode::Baked, false).unwrap();
    let live = glossy(LightMode::Realtime, false).unwrap();
    let (bx, by, b) = peak(&baked, &diffuse);
    let (lx, ly, l) = peak(&live, &diffuse);
    let (be, le) = (energy(&baked, &diffuse), energy(&live, &diffuse));
    eprintln!("[baked-spec] baked peak {b:.1} at ({bx},{by}) energy {be:.0}, realtime {l:.1} at ({lx},{ly}) energy {le:.0}");
    assert!(l > 20.0, "the realtime lamp shows a highlight: {l:.1}");
    assert!(
        bx.abs_diff(lx) <= 2 && by.abs_diff(ly) <= 2,
        "the baked highlight sits where the realtime one does"
    );
    for (what, ratio) in [("peak", b / l), ("energy", be / le)] {
        assert!(
            (1.0 / 1.5..1.5).contains(&ratio),
            "the baked {what} is within 1.5x of the realtime one: {ratio:.2}"
        );
    }
}

#[test]
fn a_rough_plane_shows_no_visible_change() {
    let mut flat = scene(LightMode::Baked, 1.0);
    bake(&mut flat, "rough_flat", false);
    let Some(flat) = shot(&flat, "rough_flat") else {
        return;
    };
    let mut lobed = scene(LightMode::Baked, 1.0);
    bake(&mut lobed, "rough_lobed", true);
    let lobed = shot(&lobed, "rough_lobed").unwrap();
    let (_, _, most) = peak(&luma(&lobed), &luma(&flat));
    eprintln!("[baked-spec] rough: at most {most:.1} brighter");
    assert!(
        most <= 4.0,
        "roughness 1 spreads the lobe to nothing: {most:.1}"
    );
}
