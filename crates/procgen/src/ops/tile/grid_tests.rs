use super::stamp::Grid;
use super::*;
use crate::ops::test_graph::{bake, bits};
use serde_json::json;

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
    assert_ne!(cell(7, 3), cell(8, 3));
    assert_eq!(cell(3, 3), cell(4, 3));
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

fn brute(img: &Image, s: &Settings) -> Image {
    let grid = Grid::new(s.count, s.aspect, s.offset);
    let ((nx, ny), per) = (grid.n, grid.per);
    let field = Field::Const(1.0);
    let cells: Vec<_> = (0..per.1)
        .flat_map(|y| (0..per.0).map(move |x| (x, y)))
        .filter_map(|c| Some((c, cell_stamp(s, &grid, (field, field), s.draws(), c)?)))
        .collect();
    Image::fill_uv(img.size(), |u, v| {
        let mut out: Rgba = [0.0, 0.0, 0.0, 1.0];
        for &((cx, cy), stamp) in &cells {
            for (tx, ty) in (-1..=1).flat_map(|y| (-1..=1).map(move |x| (x, y))) {
                let (x, y) = ((u + tx as f32) * nx, (v + ty as f32) * ny);
                let local = stamp.local(x - cx as f32 - 0.5 - grid.shift(cy), y - cy as f32 - 0.5);
                if (0.0..1.0).contains(&local.0) && (0.0..1.0).contains(&local.1) {
                    let p = img.sample_bilinear_wrapped(local.0, local.1);
                    fold(TileBlend::Max, &mut out, fill(p, stamp.value, false));
                }
            }
        }
        out
    })
}

#[test]
fn turned_planks_in_a_staggered_grid_lose_no_texel_to_the_reach() {
    let plank = Image::fill_uv(64, |u, v| {
        let inside = (0.1..0.9).contains(&u) && (0.2..0.8).contains(&v);
        [f32::from(inside), u, v, 1.0]
    });
    for rotation in [0.05, 0.3, 1.0] {
        let tile = Tile {
            aspect: [1.0, 4.0],
            offset: 0.5,
            jitter: 0.4.into(),
            rotation_jitter: rotation.into(),
            scale_jitter: 0.5.into(),
            ..Tile::new(3.0)
        };
        let s = Settings::of(&tile, &Links::default(), None, None, 4);
        assert!(
            super::tile(&plank, &s) == brute(&plank, &s),
            "rotation {rotation}"
        );
    }
}
