//! `tile`'s behaviour (#407, #1742): the plain scatter, and the Tile Sampler
//! params each keeping today's bake at their defaults.

use super::*;
use crate::procgen::hash;

/// A white square filling the whole stamp, so a stamp covers its cell exactly.
fn full(size: u32) -> Image {
    Image::fill_uv(size, |_, _| [1.0; 4])
}

/// A disc of radius 0.2 on black, so jitter 0.3 never makes two stamps overlap.
fn dot(size: u32) -> Image {
    Image::fill_uv(size, |u, v| {
        let d = ((u - 0.5).powi(2) + (v - 0.5).powi(2)).sqrt();
        [f32::from(d < 0.2), f32::from(d < 0.2), 0.0, 1.0]
    })
}

/// `tile`'s scatter with its mask and size map, nothing linked, seed 7.
fn scatter<'a>(tile: &Tile, mask: Option<&'a Image>, size: Option<&'a Image>) -> Scatter<'a> {
    Scatter::of(tile, &Links::default(), mask, size, 7)
}

fn jittered() -> Tile {
    Tile {
        jitter: 0.3.into(),
        ..Tile::new(4.0)
    }
}

/// The scatter as it was before #1742, kept to hold its defaults to the same bytes.
fn legacy(img: &Image, cells: f32, j: Jitter, seed: u64) -> Image {
    let n = count(cells);
    let per = n as i64;
    let reach = j.reach(1.0);
    Image::fill_uv(img.size(), |u, v| {
        let (px, py) = (u * n, v * n);
        let (gx, gy) = (px.floor() as i64, py.floor() as i64);
        let mut out: Rgba = [0.0, 0.0, 0.0, 1.0];
        for (dx, dy) in (-reach..=reach).flat_map(|y| (-reach..=reach).map(move |x| (x, y))) {
            let (cx, cy) = (gx + dx, gy + dy);
            let cell = (cx.rem_euclid(per), cy.rem_euclid(per));
            let r = |ch: u64| hash::unit2(cell.0, cell.1, 20 + ch, seed) - 0.5;
            let angle = j.rotation * r(2) * std::f32::consts::TAU;
            let (c, s) = (angle.cos(), angle.sin());
            let (x, y) = (
                px - cx as f32 - 0.5 - j.offset * r(0),
                py - cy as f32 - 0.5 - j.offset * r(1),
            );
            let inv = 1.0 / (1.0 + j.scale * r(3));
            let (su, sv) = ((x * c + y * s) * inv + 0.5, (-x * s + y * c) * inv + 0.5);
            if (0.0..1.0).contains(&su) && (0.0..1.0).contains(&sv) {
                let p = img.sample_bilinear_wrapped(su, sv);
                out = std::array::from_fn(|c| out[c].max(p[c]));
            }
        }
        out
    })
}

#[test]
fn the_defaults_bake_what_the_scatter_did_before_them() {
    let src = Image::fill_uv(32, |u, v| [u, v * u, 1.0 - v, 1.0]);
    let tile = Tile {
        jitter: 0.6.into(),
        rotation_jitter: 0.4.into(),
        scale_jitter: 0.8.into(),
        ..Tile::new(5.0)
    };
    let new = super::tile(&src, &scatter(&tile, None, None));
    assert!(new == legacy(&src, 5.0, Jitter::new(0.6, 0.4, 0.8), 7));
}

#[test]
fn each_cell_reads_its_own_jitter_from_a_link() {
    // A 4×4 grid over a 16×16 link: cell (cx, cy) is a 4×4 block of red
    // (cx + 4cy) / 16, so a read anywhere but that cell's centre is off.
    let link = Image::fill_uv(16, |u, v| {
        let (cx, cy) = ((u * 4.0).floor(), (v * 4.0).floor());
        [(cx + 4.0 * cy) / 16.0, 0.0, 0.0, 1.0]
    });
    let field = Field::Map {
        img: &link,
        min: 0.0,
        max: 1.0,
    };
    let jitters = Jitters {
        offset: field,
        ..Jitters::uniform(0.0, 0.0, 0.0)
    };
    for (cx, cy) in (0..4).flat_map(|y| (0..4).map(move |x| (x, y))) {
        let want = (cx + 4 * cy) as f32 / 16.0;
        assert_eq!(jitters.at((cx, cy), 4.0).offset, want, "cell ({cx}, {cy})");
    }
}

#[test]
fn no_jitter_repeats_the_input_exactly() {
    let src = Image::fill_uv(8, |u, v| [u, v, 0.0, 1.0]);
    let out = super::tile(
        &src,
        &Scatter::plain(2.0, Jitters::uniform(0.0, 0.0, 0.0), 1),
    );
    for (x, y) in (0..8).flat_map(|y| (0..8).map(move |x| (x, y))) {
        let at = |c: i64| ((c % 4) as f32 + 0.5) / 4.0;
        let expect = src.sample_bilinear_wrapped(at(x), at(y));
        assert_eq!(out.get_wrapped(x, y), expect, "({x},{y})");
    }
}

/// The value at the centre pixel of each cell of an 8×8 grid baked at `res`².
fn centres(img: &Image, res: i64) -> Vec<f32> {
    let at = |c: i64| c * res / 8 + res / 16;
    (0..64)
        .map(|i| img.get_wrapped(at(i % 8), at(i / 8))[0])
        .collect()
}

#[test]
fn random_fills_about_half_the_cells_each_with_its_own_flat_grey() {
    let tile = Tile {
        output: TileOutput::Random,
        probability: 0.5,
        ..Tile::new(8.0)
    };
    let out = super::tile(&full(64), &scatter(&tile, None, None));
    assert!(
        out == super::tile(&full(64), &scatter(&tile, None, None)),
        "deterministic"
    );
    for (x, y) in (0..64).flat_map(|y| (0..64).map(move |x| (x, y))) {
        let corner = out.get_wrapped(x / 8 * 8, y / 8 * 8)[0];
        assert_eq!(out.get_wrapped(x, y)[0], corner, "one flat grey per cell");
    }
    let mut greys: Vec<f32> = centres(&out, 64).into_iter().filter(|&g| g > 0.0).collect();
    let filled = greys.len();
    assert!((20..=44).contains(&filled), "{filled} of 64 cells filled");
    greys.sort_by(f32::total_cmp);
    greys.dedup();
    assert_eq!(greys.len(), filled, "the greys differ between stamps");
}

#[test]
fn probability_and_value_jitter_keep_every_stamp_where_it_was() {
    let base = super::tile(&dot(32), &scatter(&jittered(), None, None));
    let sparse = Tile {
        probability: 0.5,
        value_jitter: 0.5,
        ..jittered()
    };
    let out = super::tile(&dot(32), &scatter(&sparse, None, None));
    let (mut kept, mut dropped) = (0, 0);
    for (b, o) in base.pixels().iter().zip(out.pixels()) {
        match (b[1] > 0.0, o[1] > 0.0) {
            (true, true) => kept += 1,
            (true, false) => dropped += 1,
            (false, false) => {}
            (false, true) => panic!("a stamp moved"),
        }
        let ratio = o[1] / b[1];
        assert!(
            o[1] == 0.0 || (0.75..=1.25).contains(&ratio),
            "value × {ratio}"
        );
    }
    assert!(kept > 0 && dropped > 0, "{kept} kept, {dropped} dropped");
}

#[test]
fn a_mask_and_a_size_map_read_at_the_cell_centre_at_any_resolution() {
    // Mask: the 8-cell grid's even columns; size map: 0.5 in the lower half.
    let mask = |res| {
        Image::fill_uv(res, |u, _| {
            [f32::from((u * 8.0) as i32 % 2 == 0), 0.0, 0.0, 1.0]
        })
    };
    let size = |res| Image::fill_uv(res, |_, v| [if v < 0.5 { 1.0 } else { 0.5 }, 0.0, 0.0, 1.0]);
    // Probe each cell a third of a cell off its centre: inside a full stamp, outside a half one.
    let bake = |res: u32| {
        let (m, s) = (mask(res), size(res));
        let out = super::tile(&full(res), &scatter(&Tile::new(8.0), Some(&m), Some(&s)));
        let r = res as i64;
        let at = |c: i64| c * r / 8 + r / 16 + r / 24;
        (0..64)
            .map(|i| out.get_wrapped(at(i % 8), at(i / 8))[0])
            .collect::<Vec<_>>()
    };
    let small = bake(128);
    assert_eq!(small, bake(1024));
    for (i, &v) in small.iter().enumerate() {
        let want = i % 2 == 0 && i / 8 < 4;
        assert_eq!(v > 0.0, want, "cell {i}");
    }
}

#[test]
fn blends_differ_only_where_stamps_overlap() {
    let grown = Image::fill_uv(64, |_, _| [1.5, 0.0, 0.0, 1.0]);
    let bake = |output, blend| {
        let tile = Tile {
            output,
            blend,
            ..Tile::new(4.0)
        };
        super::tile(&full(64), &scatter(&tile, None, Some(&grown)))
    };
    let index_max = bake(TileOutput::Index, TileBlend::Max);
    // Index rises in row-major order, so the latest stamp is the brightest.
    assert!(index_max == bake(TileOutput::Index, TileBlend::Over));
    let (max, over) = (
        bake(TileOutput::Random, TileBlend::Max),
        bake(TileOutput::Random, TileBlend::Over),
    );
    let add = bake(TileOutput::Random, TileBlend::Add);
    assert!(max != over, "over keeps the latest, not the brightest");
    for ((m, o), a) in max.pixels().iter().zip(over.pixels()).zip(add.pixels()) {
        assert!(
            o[0] <= m[0] && a[0] >= m[0],
            "max {m:?} over {o:?} add {a:?}"
        );
    }
}

#[test]
fn out_of_range_params_are_capped_or_ignored() {
    let base = super::tile(&dot(16), &scatter(&jittered(), None, None));
    for (p, vj) in [(f32::NAN, f32::NAN), (f32::INFINITY, 0.0), (1e30, -1e30)] {
        let tile = Tile {
            probability: p,
            value_jitter: vj,
            ..jittered()
        };
        assert!(
            super::tile(&dot(16), &scatter(&tile, None, None)) == base,
            "{p} {vj}"
        );
    }
    let huge = Image::fill_uv(16, |u, _| {
        [if u < 0.5 { f32::NAN } else { 1e30 }, 0.0, 0.0, 1.0]
    });
    let tile = Tile {
        value_jitter: f32::INFINITY,
        probability: f32::INFINITY,
        ..Tile::new(1e9)
    };
    super::tile(&dot(16), &scatter(&tile, Some(&huge), Some(&huge)));
}
