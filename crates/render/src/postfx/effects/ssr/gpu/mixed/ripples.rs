//! A rippled translucent pool under *Mixed*: no lone ray's dot, no ray under the water.

use super::super::fixture::{paint, place, MIRROR, W};
use super::super::transparent::busy_normal_map;
use super::{mean, room, shot, ABOVE};
use glam::Vec3;
use rusty_components::{PostProcessingComponent, RenderMode, Ssr, SsrTracing};
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::Scene;

fn rippled(
    map: String,
    tiling: [f32; 2],
    tune: impl FnOnce(&mut PostProcessingComponent),
) -> Scene {
    let mut s = room(MIRROR, true, tune);
    let floor = s.materials.get_mut("floor").unwrap();
    (floor.render_mode, floor.alpha, floor.receive_ssr) =
        (RenderMode::Transparent, 0.5, Some(true));
    (floor.normal_map, floor.tiling) = (Some(map.into()), tiling);
    s.materials.get_mut("cube").unwrap().emissive = [20.0; 3];
    let cube = s.find_entity_by_name("Cube").unwrap();
    place(&mut s, cube, ABOVE, Vec3::new(2.0, 0.2, 2.0));
    s
}

fn marched(v: &mut PostProcessingComponent) {
    v.ssr.tracing = SsrTracing::RayMarching;
}

fn luma(frame: &[u8], x: u32, y: u32) -> f32 {
    let i = ((y * W + x) * 4) as usize;
    0.2126 * f32::from(frame[i])
        + 0.7152 * f32::from(frame[i + 1])
        + 0.0722 * f32::from(frame[i + 2])
}

/// Pixels brighter by `by` than the brightest of eight 3 px off: a half-size texel lit alone.
fn dots(frame: &[u8], by: f32) -> usize {
    let h = frame.len() as u32 / 4 / W;
    let ring = [
        (-3, -3),
        (0, -3),
        (3, -3),
        (-3, 0),
        (3, 0),
        (-3, 3),
        (0, 3),
        (3, 3),
    ];
    let all = (3..h - 3).flat_map(|y| (3..W - 3).map(move |x| (x, y)));
    all.filter(|&(x, y)| {
        let at = |(dx, dy): (i32, i32)| {
            luma(
                frame,
                x.saturating_add_signed(dx),
                y.saturating_add_signed(dy),
            )
        };
        let around = ring.map(at).into_iter().fold(0.0, f32::max);
        luma(frame, x, y) > around + by
    })
    .count()
}

fn mean_luma(frame: &[u8]) -> f32 {
    let n = frame.len() / 4;
    let all = (0..n).map(|p| luma(frame, p as u32 % W, p as u32 / W));
    all.sum::<f32>() / n as f32
}

// Dots: M4 0, CI's Metal 1, unbounded 49; a lamp two rays reach is kept. Luma 200.2 vs 191.5.
#[test]
fn gpu_a_rippled_pool_under_mixed_shows_no_lone_bright_dot() {
    let Some(mut r) = super::super::fixture::renderer() else {
        return;
    };
    let march = shot(&mut r, &rippled(busy_normal_map(), [5.0, 7.5], marched));
    let mixed = shot(&mut r, &rippled(busy_normal_map(), [5.0, 7.5], |_| {}));
    let (n, lit, dark) = (dots(&mixed, 40.0), mean_luma(&mixed), mean_luma(&march));
    eprintln!("[ssr] rippled pool: {n} dots, mean luma {lit:.1} (RayMarching {dark:.1})");
    assert!(n <= 4, "a ray that alone reached the lamp is a dot: {n}");
    assert!(
        lit > dark + 4.0,
        "the rays still reflect the lamp: {lit} vs {dark}"
    );
}

/// Leaning 70 degrees, seen 70 degrees down: every reflection turns under the surface.
fn leaning_normal_map() -> String {
    let (x, y) = (70f32.to_radians().sin(), 70f32.to_radians().cos());
    let texel = [
        127,
        (x * 127.5 + 127.5) as u8,
        (y * 127.5 + 127.5) as u8,
        255,
    ];
    let img = image::RgbaImage::from_pixel(4, 4, image::Rgba(texel));
    let path = rusty_test_support::temp::dir().join("rusty_2658_leaning.png");
    img.save(&path).unwrap();
    path.to_string_lossy().into_owned()
}

/// Over a red basin a metre down, the march one short step: only a ray could reach it.
fn over_basin(tune: impl FnOnce(&mut PostProcessingComponent)) -> Scene {
    let mut s = rippled(leaning_normal_map(), [1.0, 1.0], |v| {
        v.ssr.quality = rusty_core::settings::quality::EffectQuality::Custom;
        (v.ssr.steps, v.ssr.step_length) = (1, Ssr::MIN_STEP_LENGTH);
        tune(v);
    });
    let basin = create_entity(&mut s, "Basin", Some(Primitive::Box));
    place(
        &mut s,
        basin,
        Vec3::new(0.0, -1.0, -10.0),
        Vec3::new(60.0, 0.1, 60.0),
    );
    paint(&mut s, basin, "basin", ([0.0; 3], 0.0, 1.0));
    s.materials.get_mut("basin").unwrap().emissive = [4.0, 0.0, 0.0];
    s.world.set_static(basin, true);
    s
}

#[test]
fn gpu_a_ray_the_ripple_turns_under_the_pool_traces_nothing() {
    let Some(mut r) = super::super::fixture::renderer() else {
        return;
    };
    let march = shot(&mut r, &over_basin(marched));
    let mixed = shot(&mut r, &over_basin(|_| {}));
    let (lit, dark) = (mean(&mixed), mean(&march));
    eprintln!("[ssr] leaning pool: {lit:?} (RayMarching {dark:?})");
    // Unruled, Metal drew [253, 147, 147] against [168, 130, 140].
    assert!(mixed == march, "no ray reflects the basin under the pool");
}
