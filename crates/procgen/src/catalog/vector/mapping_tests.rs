//! A positive turn turns the content counter-clockwise as shown (v down); a translation moves
//! the content by its negative.
use super::*;
use crate::catalog::filter::test_probe::{relief, rolled, run};
use crate::recipe::OpKind;

const N: i64 = 32;

fn mapping(scale: [f32; 2], rotation: f32, translation: [f32; 2], tiling: bool) -> OpKind {
    OpKind::Mapping(Mapping {
        scale,
        rotation,
        translation,
        tiling,
    })
}

fn dot((x, y): (i64, i64)) -> Image {
    let mut img = Image::filled(N as u32, [0.0, 0.0, 0.0, 1.0]);
    img.pixels_mut()[(y.rem_euclid(N) * N + x.rem_euclid(N)) as usize] = [1.0; 4];
    img
}

fn turned((x, y): (i64, i64), quarters: i64) -> (i64, i64) {
    match quarters.rem_euclid(4) {
        0 => (x, y),
        1 => (y, N - 1 - x),
        2 => (N - 1 - x, N - 1 - y),
        _ => (N - 1 - y, x),
    }
}

fn assert_near(a: &Image, b: &Image, what: &str) {
    crate::catalog::filter::test_probe::assert_close(a, b, 1e-4, what);
}

#[test]
fn a_turn_moves_the_mark_about_the_centre_counter_clockwise() {
    // Top right of the tile: a quarter turn takes it to the top left.
    let mark = (24, 4);
    assert_eq!(turned(mark, 1), (4, 7));
    for (turns, quarters) in [(0.25, 1), (0.5, 2), (0.75, 3), (-0.25, 3), (1.0, 0)] {
        let out = run(&mapping([1.0; 2], turns, [0.0; 2], false), &dot(mark));
        assert_near(
            &out,
            &dot(turned(mark, quarters)),
            &format!("{turns} turns"),
        );
    }
}

/// Channel 0 is constant on each pair of pixels along `axis`, so a half-way read is exact.
fn pairs(axis: usize) -> Image {
    Image::fill_uv(N as u32, |u, v| {
        let (along, across) = if axis == 0 { (u, v) } else { (v, u) };
        [(along * 16.0).floor() / 16.0, across, 0.0, 1.0]
    })
}

#[test]
fn a_scale_of_two_repeats_twice_along_its_axis_and_leaves_the_other() {
    for axis in 0..2 {
        let mut scale = [1.0; 2];
        scale[axis] = 2.0;
        let img = pairs(axis);
        let out = run(&mapping(scale, 0.0, [0.0; 2], true), &img);
        // About the centre: output pixel c reads input pixel 2c − N/2.
        let want = Image::fill_uv(N as u32, |u, v| {
            let (x, y) = (u * N as f32, v * N as f32);
            let mut at = [x as i64, y as i64];
            at[axis] = 2 * at[axis] - N / 2;
            img.get_wrapped(at[0], at[1])
        });
        assert_eq!(out, want, "axis {axis}");
        let half = [(N / 2, 0), (0, N / 2)][axis];
        assert_eq!(rolled(&out, half.0, half.1), out, "axis {axis} repeats");
    }
}

#[test]
fn a_translation_moves_the_content_back_by_that_much_of_a_tile_wrapped() {
    let mark = (24, 4);
    for t in [
        [0.5, 0.0],
        [0.25, 0.0],
        [0.0, 0.25],
        [-0.25, 0.5],
        [1.25, 0.0],
    ] {
        let out = run(&mapping([1.0; 2], 0.0, t, true), &dot(mark));
        let shift = t.map(|t| (t * N as f32) as i64);
        let want = dot((mark.0 - shift[0], mark.1 - shift[1]));
        assert_eq!(out, want, "translation {t:?}");
    }
}

#[test]
fn a_translation_moves_the_content_before_the_turn() {
    // The read point is the output point turned, scaled, then offset.
    let out = run(&mapping([1.0; 2], 0.25, [0.25, 0.0], true), &dot((24, 4)));
    assert_eq!(out, dot(turned((16, 4), 1)));
}

#[test]
fn tiling_snaps_a_turn_to_the_nearest_quarter() {
    let mark = (24, 4);
    let cases = [
        (0.2, 1),
        (0.3, 1),
        (0.45, 2),
        (0.7, 3),
        (0.9, 0),
        (-0.2, 3),
        (-0.6, 2),
    ];
    for (turns, quarters) in cases {
        let out = run(&mapping([1.0; 2], turns, [0.0; 2], true), &dot(mark));
        assert_eq!(out, dot(turned(mark, quarters)), "{turns} turns");
    }
}

#[test]
fn tiling_snaps_a_scale_to_a_whole_count() {
    let img = relief(N as u32);
    for (given, whole) in [(1.6, 2.0), (2.4, 2.0), (0.3, 1.0), (2.6, 3.0)] {
        let snapped = run(&mapping([given, given], 0.0, [0.0; 2], true), &img);
        let exact = run(&mapping([whole, whole], 0.0, [0.0; 2], false), &img);
        assert_eq!(snapped, exact, "scale {given}");
    }
}

fn seam_and_step(img: &Image) -> (f32, f32) {
    let (n, at) = (img.width() as i64, |x, y| img.get_wrapped(x, y)[0]);
    let (mut seam, mut step) = (0.0f32, 0.0f32);
    for a in 0..n {
        seam = seam.max((at(0, a) - at(n - 1, a)).abs());
        seam = seam.max((at(a, 0) - at(a, n - 1)).abs());
        for b in 0..n - 1 {
            step = step.max((at(b, a) - at(b + 1, a)).abs());
            step = step.max((at(a, b) - at(a, b + 1)).abs());
        }
    }
    (seam, step)
}

#[test]
fn a_tiled_mapping_still_tiles_and_a_free_one_opens_a_seam() {
    let img = relief(64);
    let op = |tiling| mapping([1.6, 2.4], 0.2, [0.1, 0.3], tiling);
    let (seam, step) = seam_and_step(&run(&op(true), &img));
    assert!(seam <= step * 1.01, "tiled: seam {seam} vs step {step}");
    let (seam, step) = seam_and_step(&run(&op(false), &img));
    assert!(seam > step * 2.0, "free: seam {seam} vs step {step}");
}
