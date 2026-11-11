//! A rough metal floor's SSR fades out (#2004): the reflection of a cube on it
//! thins away down the floor, where a hit drawn inside a hard silhouette made a
//! block with a sharp edge; and its reflection of a cube lifted off it has soft
//! sides (#2028), where a hit drawn at full weight or none stepped at one pixel.

use glam::Vec3;

use super::silhouette::{at, base, shot, BRONZE, H, W};
use crate::scene::Camera;

/// How hard the edge is where the cube's reflection on the floor ends: along the
/// redness averaged over the cube's middle columns, from just under the cube to the
/// frame's foot, the most one row's step exceeds both its neighbours'. A fade spreads
/// its change over rows; an edge puts it in one.
fn hardest_edge(on: &[u8], off: &[u8]) -> i32 {
    let red = |f: &[u8], y: u32| -> i32 {
        (W / 2 - 12..W / 2 + 12)
            .map(|x| at(f, x, y))
            .map(|c| c[0] - c[2])
            .sum::<i32>()
            / 24
    };
    let foot = (0..H)
        .filter(|&y| red(off, y) > 60)
        .max()
        .expect("the cube is in view");
    let steps: Vec<i32> = (foot + 2..H - 1)
        .map(|y| (red(on, y + 1) - red(on, y)).abs())
        .collect();
    steps
        .windows(3)
        .map(|w| w[1] - w[0].max(w[2]))
        .max()
        .unwrap()
}

#[test]
fn gpu_a_rough_metal_floor_fades_the_cubes_reflection_without_a_hard_edge() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let down = Camera::new(Vec3::new(0.0, 1.0, 0.0), -90.0, -12.0);
    let floor = |on| base(on, (BRONZE, 1.0, 0.35));
    let (on, off) = (
        shot(&mut r, &floor(true), &down),
        shot(&mut r, &floor(false), &down),
    );
    let (lit, plain) = (hardest_edge(&on, &off), hardest_edge(&off, &off));
    eprintln!("[ssr] rough metal: hardest edge under the cube {lit}, SSR off {plain}");
    assert!(lit <= plain + 3, "the reflection fades out, with no edge");
}

/// The largest share of the reflection's redness one pixel adds across its left
/// side, in the row of the floor whose middle reflects the most: near 1 is a hard
/// outline, a soft one spreads its rise over its blur.
fn side_step(on: &[u8], off: &[u8]) -> f32 {
    let red = |f: &[u8], x: u32, y: u32| at(f, x, y)[0] - at(f, x, y)[2];
    let added = |y: u32, x: u32| red(on, x, y) - red(off, x, y);
    let row = (H / 2..H).max_by_key(|&y| added(y, W / 2)).unwrap();
    let peak = added(row, W / 2).max(1) as f32;
    (1..=W / 2)
        .map(|x| added(row, x) - added(row, x - 1))
        .max()
        .unwrap() as f32
        / peak
}

/// The rough floor's scene, its cube lifted 0.6 m: a reflection a long ray away,
/// as a shelf is from a soffit above it (#2028).
fn lifted(on: bool, roughness: f32) -> crate::scene::Scene {
    let mut s = base(on, (BRONZE, 1.0, roughness));
    let cube = s.find_entity_by_name("Cube").unwrap();
    s.world.transform_mut(cube).unwrap().position.y += 0.6;
    s
}

#[test]
fn gpu_a_satin_metal_floor_reflects_a_lifted_cube_with_soft_sides() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let down = Camera::new(Vec3::new(0.0, 1.0, 0.0), -90.0, -12.0);
    let mut step = |roughness| {
        let (on, off) = (
            shot(&mut r, &lifted(true, roughness), &down),
            shot(&mut r, &lifted(false, roughness), &down),
        );
        side_step(&on, &off)
    };
    let (mirror, satin) = (step(0.04), step(0.35));
    eprintln!("[ssr] the lifted cube's reflected side: at most {mirror:.2} of its redness in one pixel on a mirror, {satin:.2} at roughness 0.35");
    assert!(mirror >= 0.5, "a mirror reflects a sharp side");
    assert!(
        satin <= 0.3,
        "a satin metal's reflection has no hard outline (#2028)"
    );
}
