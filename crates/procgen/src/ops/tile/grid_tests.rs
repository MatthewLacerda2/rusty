//! `tile`'s grid (#2187): `aspect` sets the cells across and down on their own, and
//! `offset` staggers every other row, its row count even so the stagger closes at
//! the wrap.

use super::stamp::Grid;
use super::*;
use crate::ops::test_graph::{bake, bits};
use serde_json::json;

/// Each stamp filled with its cell's index, so a pixel names the cell it is in;
/// baked at `size`² (the stamp's own size).
fn index_bake(tile: Tile, size: u32) -> Image {
    let full = Image::fill_uv(size, |_, _| [1.0; 4]);
    let tile = Tile {
        output: TileOutput::Index,
        ..tile
    };
    super::tile(
        &full,
        &Settings::of(&tile, &Links::default(), None, None, 1),
    )
}

#[test]
fn aspect_sets_the_cells_across_and_down_on_their_own() {
    let g = Grid::new(4.0, [1.0, 4.0], 0.0);
    assert_eq!(g.per, (4, 16));
    assert_eq!(
        Grid::new(3.0, [1.4, 0.5], 0.0).per,
        (4, 2),
        "each axis rounds"
    );
    let tile = Tile {
        aspect: [1.0, 4.0],
        ..Tile::new(4.0)
    };
    let out = index_bake(tile, 16);
    // A 16² bake of a 4 × 16 grid: pixel (x, y) is in cell (x / 4, y).
    for (x, y) in (0..16).flat_map(|y| (0..16).map(move |x| (x, y))) {
        let want = ((y * 4 + x / 4 + 1) as f64 / 64.0) as f32;
        assert_eq!(out.get_wrapped(x, y)[0], want, "({x}, {y})");
    }
}

#[test]
fn an_offset_staggers_odd_rows_and_rounds_the_rows_to_even() {
    let g = Grid::new(4.0, [1.0, 1.25], 0.5);
    assert_eq!(g.per, (4, 6), "5 rows round to an even 6");
    assert_ne!(
        g.shift(g.per.1 - 1),
        g.shift(0),
        "the stagger closes at the wrap"
    );
    assert_eq!(
        Grid::new(4.0, [1.0, 1.25], 0.0).per,
        (4, 5),
        "no offset, no rounding"
    );
    assert_eq!(Grid::new(4.0, [1.0, 1.0], f32::NAN).shift, 0.0);
    assert_eq!(Grid::new(4.0, [1.0, 1.0], 7.0).shift, 1.0);
}

#[test]
fn a_running_bond_shifts_each_odd_row_half_a_cell_across_the_wrap() {
    let tile = Tile {
        offset: 0.5,
        ..Tile::new(4.0)
    };
    let out = index_bake(tile, 32);
    let cell = |x: i64, y: i64| out.get_wrapped(x, y)[0];
    // Cells 8 pixels wide and tall, 4 across. Even row 0: cells start at x = 0, 8, ….
    assert_ne!(cell(7, 3), cell(8, 3));
    assert_eq!(cell(3, 3), cell(4, 3));
    // Odd row 1: cells start at x = 4, 12, …, and the one across the wrap
    // (x = 28..32 and 0..4) is one stamp.
    assert_ne!(cell(3, 11), cell(4, 11));
    assert_eq!(cell(7, 11), cell(8, 11));
    assert_eq!(
        cell(31, 11),
        cell(0, 11),
        "the shifted stamp crosses the wrap"
    );
    assert_eq!(
        cell(0, 11),
        ((4 + 3 + 1) as f32) / 16.0,
        "it is row 1's last cell"
    );
}

#[test]
fn a_staggered_jittered_grid_reads_its_params_from_a_recipe_and_is_seeded() {
    let doc = |seed| {
        json!({ "op": "tile", "count": 3, "aspect": [2, 1], "offset": 0.5,
                "jitter": 0.5, "rotation_jitter": 0.3, "seed": seed })
    };
    let a = bake(doc(1), 1, vec![], [64, 32]);
    assert!(bits(&a) == bits(&bake(doc(1), 1, vec![], [64, 32])));
    assert!(bits(&a) != bits(&bake(doc(2), 1, vec![], [64, 32])));
}
