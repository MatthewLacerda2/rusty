use glam::{Vec2, Vec3};

use super::fixtures::{bake_with, cube, l_shape, quad};
use super::{MeshMap, MeshSource, Settings};
use rusty_procgen::image_buf::Image;

const S: u32 = 256;
const SLAB: (f32, f32) = (0.5, 0.55);
/// The bolt head's half-width and its height.
const BOLT: f32 = 0.03;
const REACH: f32 = 0.05;

/// A box of `cube`'s outward faces but `skip`, face `k` drawn at UV rectangle `uv(k)`.
fn block(
    m: &mut MeshSource,
    (lo, hi): (Vec3, Vec3),
    skip: &[usize],
    uv: &mut impl FnMut(usize) -> (Vec2, Vec2),
) {
    for (k, f) in cube().positions.chunks_exact(4).enumerate() {
        if !skip.contains(&k) {
            let c = [f[0], f[1], f[2], f[3]].map(|p| lo + p * (hi - lo));
            quad(m, c, uv(k), 0);
        }
    }
}

/// A 1 m slab on four legs over a floor plate, a 6 cm bolt head at its centre: the slab's top
/// fills UV0's lower-left quarter, its underside the lower-right.
fn table() -> MeshSource {
    let mut m = MeshSource::default();
    let v = Vec3::new;
    let (bottom, top) = SLAB;
    let mut next = 0;
    let mut cell = |_| {
        let lo = Vec2::new((next % 8) as f32, (next / 8) as f32) / 8.0 + Vec2::new(0.0, 0.5);
        next += 1;
        (lo + 0.01, lo + 0.115)
    };
    let mut slab = |k: usize| match k {
        0 => (Vec2::ZERO, Vec2::splat(0.5)),
        1 => (Vec2::new(0.5, 0.0), Vec2::new(1.0, 0.5)),
        _ => (Vec2::ZERO, Vec2::ZERO),
    };
    block(
        &mut m,
        (v(0., bottom, 0.), v(1., top, 1.)),
        &[2, 3, 4, 5],
        &mut slab,
    );
    block(
        &mut m,
        (v(0., bottom, 0.), v(1., top, 1.)),
        &[0, 1],
        &mut cell,
    );
    for (x, z) in [(0.0, 0.0), (0.95, 0.0), (0.0, 0.95), (0.95, 0.95)] {
        block(
            &mut m,
            (v(x, 0.0, z), v(x + 0.05, bottom, z + 0.05)),
            &[0, 1],
            &mut cell,
        );
    }
    let floor = [
        v(-2., 0., -2.),
        v(-2., 0., 3.),
        v(3., 0., 3.),
        v(3., 0., -2.),
    ];
    quad(&mut m, floor, cell(0), 0);
    let (a, b) = (0.5 - BOLT, 0.5 + BOLT);
    block(&mut m, (v(a, top, a), v(b, top + BOLT, b)), &[1], &mut cell);
    m
}

fn ao(mesh: &MeshSource, ao_distance: Option<f32>) -> Image {
    let settings = Settings {
        ao_distance,
        ..Settings::new(S)
    };
    bake_with(mesh, &[MeshMap::Ao], &settings)
        .unwrap()
        .remove(0)
        .1
}

/// The mean `ao` of the texels whose centres lie in a UV rectangle.
fn mean(img: &Image, lo: Vec2, hi: Vec2) -> f32 {
    let (lo, hi) = ((lo * S as f32).ceil(), (hi * S as f32).floor());
    let texels: Vec<f32> = (lo.y as u32..hi.y as u32)
        .flat_map(|y| (lo.x as u32..hi.x as u32).map(move |x| (x, y)))
        .map(|(x, y)| img.pixels()[(y * S + x) as usize][0])
        .collect();
    texels.iter().sum::<f32>() / texels.len() as f32
}

/// The slab top's UV for a point on it: `quad` runs U along its first edge, here `z`.
fn on_top(x: f32, z: f32) -> Vec2 {
    Vec2::new(z, x) * 0.5
}

#[test]
fn a_short_ao_distance_opens_the_slab_underside_and_keeps_the_bolt_crease() {
    let table = table();
    let (whole, short) = (ao(&table, None), ao(&table, Some(REACH)));
    let under = |img| mean(img, Vec2::new(0.6, 0.1), Vec2::new(0.9, 0.4));
    let (dark, open) = (under(&whole), under(&short));
    assert!(dark < 0.2, "the default underside reads {dark}");
    assert!(open > 0.999, "a 5 cm reach reads the underside {open}");
    // Two texels (8 mm) wide, beside the bolt's +X side.
    let foot = |img| {
        mean(
            img,
            on_top(0.5 + BOLT, 0.5 - BOLT),
            on_top(0.5 + BOLT + 0.02, 0.5 + BOLT),
        )
    };
    let (kept, before) = (foot(&short), foot(&whole));
    assert!(kept < 0.85, "the bolt's foot reads {kept}");
    assert!(
        (kept - before).abs() < 0.05,
        "the crease {kept} at 5 cm, {before} at the diagonal"
    );
    let far = mean(&short, on_top(0.2, 0.2), on_top(0.3, 0.3));
    assert!(far > 0.999, "the open top reads {far}");
}

#[test]
fn an_ao_distance_of_the_diagonal_bakes_the_default_map_bit_for_bit() {
    let l = l_shape();
    let diagonal = 3f32.sqrt();
    assert_eq!(ao(&l, None).pixels(), ao(&l, Some(diagonal)).pixels());
}
