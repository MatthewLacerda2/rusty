//! `over` composites (#1742's review): a faint edge never cuts a trench into what
//! is under it, a lone stamp bakes as under `max`, and alpha is left as `max` leaves it.

use super::super::*;
use super::Hits;

/// A soft disc: red falls from 1 at the centre to 0 at radius 0.4, so every
/// stamp has a wide faint rim.
fn soft(size: u32) -> Image {
    Image::fill_uv(size, |u, v| {
        let d = ((u - 0.5).powi(2) + (v - 0.5).powi(2)).sqrt();
        let r = ((0.4 - d) / 0.25).clamp(0.0, 1.0);
        [r, r * 0.5, 0.0, 1.0]
    })
}

fn bake(stamp: &Image, tile: Tile, size: Option<&Image>) -> Image {
    let white = Image::fill_uv(stamp.size(), |_, _| [1.0; 4]);
    let mask = size.map(|_| &white);
    super::super::tile(stamp, &Scatter::of(&tile, &Links::default(), mask, size, 3))
}

#[test]
fn a_faint_later_edge_keeps_what_is_under_it() {
    let mut hits = Hits::default();
    hits.push([0, 1, 0, 1], [0.01, 0.01, 0.01, 1.0], 0.01);
    hits.push([0, 0, 0, 0], [1.0, 1.0, 1.0, 1.0], 1.0);
    let mut out = [0.0, 0.0, 0.0, 1.0];
    hits.composite(&mut out);
    assert!((out[0] - 1.0).abs() < 1e-6, "{out:?}");
}

#[test]
fn overlapping_soft_stamps_leave_no_trench() {
    // Two cells, each stamp grown 1.8×, so their rims cross the earlier stamps'
    // bright middles. With a grey stamp, over is a screen: never below max.
    let grown = Image::fill_uv(64, |_, _| [1.8, 0.0, 0.0, 1.0]);
    let tile = |blend| Tile {
        blend,
        output: TileOutput::Index,
        ..Tile::new(2.0)
    };
    let max = bake(&soft(64), tile(TileBlend::Max), Some(&grown));
    let over = bake(&soft(64), tile(TileBlend::Over), Some(&grown));
    let mut rim = 0;
    for (m, o) in max.pixels().iter().zip(over.pixels()) {
        // Index greys are 0.25–1, so over may sit below max only where a dimmer
        // stamp covers fully; a trench is a pixel near zero under a bright max.
        assert!(o[0] >= 0.2 * m[0], "trench: max {m:?} over {o:?}");
        rim += usize::from(o[0] > m[0]);
    }
    assert!(rim > 0, "the stamps overlap somewhere");
}

#[test]
fn coverage_reads_red_in_the_grey_outputs_and_the_brightest_in_stamp() {
    let p = [0.0, 0.8, 0.2, 0.5];
    assert_eq!(coverage(p, true), 0.0);
    assert_eq!(coverage(p, false), 0.4);
}

#[test]
fn a_lone_stamp_bakes_the_same_bytes_under_every_blend() {
    for output in [TileOutput::Stamp, TileOutput::Random, TileOutput::Index] {
        let tile = |blend| Tile {
            blend,
            output,
            jitter: 0.2.into(),
            rotation_jitter: 0.5.into(),
            ..Tile::new(3.0)
        };
        let max = bake(&soft(48), tile(TileBlend::Max), None);
        assert!(
            max == bake(&soft(48), tile(TileBlend::Add), None),
            "{output:?} add"
        );
        assert!(
            max == bake(&soft(48), tile(TileBlend::Over), None),
            "{output:?} over"
        );
    }
}

#[test]
fn over_leaves_alpha_as_max_does() {
    let half = Image::fill_uv(32, |u, _| [1.0, 1.0, 1.0, if u < 0.6 { 0.5 } else { 0.0 }]);
    // No jitter, so no overlap: each pixel meets one stamp at most.
    let tile = |blend| Tile {
        blend,
        ..Tile::new(4.0)
    };
    let (max, over) = (
        bake(&half, tile(TileBlend::Max), None),
        bake(&half, tile(TileBlend::Over), None),
    );
    for (m, o) in max.pixels().iter().zip(over.pixels()) {
        assert_eq!(o[3], m[3]);
        assert!(o[0] <= 0.5 + 1e-6, "colour premultiplied by alpha: {o:?}");
    }
}
