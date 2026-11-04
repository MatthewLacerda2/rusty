//! The atlas packer (#438, #1694): every lightmap lands whole, inside one page, in
//! a cell no other overlaps, with its texels where its scale/offset says and its
//! edge extended over its cell; the smallest page that fits is chosen; maps at the
//! cap pair on a page; and pages are filled before new ones open.

use glam::Vec3;

use super::*;

/// A `size`² lightmap for `entity`, every texel `value`.
fn map(entity: u32, size: u32, value: f32) -> Lightmap {
    Lightmap {
        entity,
        size,
        texels: vec![Vec3::splat(value); (size * size) as usize],
        directions: Vec::new(),
    }
}

/// A placement's cell on its page, in texels: `(page, x0, y0, x1, y1)`.
fn cell(atlas: &LightmapAtlas, i: usize, size: u32) -> (u32, u32, u32, u32, u32) {
    let (_, page, st) = atlas.placements[i];
    let edge = atlas.page_size as f32;
    let (x, y) = ((st[2] * edge) as u32 - PAD, (st[3] * edge) as u32 - PAD);
    let e = cell_edge(size);
    (page, x, y, x + e, y + e)
}

#[test]
fn small_bakes_share_the_smallest_page_that_fits() {
    let maps = [map(1, 16, 1.0), map(2, 8, 2.0), map(3, 16, 3.0)];
    let atlas = pack(&maps);
    assert_eq!((atlas.page_size, atlas.pages.len()), (64, 1));
    for (i, m) in maps.iter().enumerate() {
        assert_eq!(atlas.placements[i].0, m.entity);
        // The whole cell, ring and slack included, holds the map's own light.
        let (page, x0, y0, x1, y1) = cell(&atlas, i, m.size);
        assert!(x0 % PAD == 0 && y0 % PAD == 0, "aligned to the grid");
        for y in y0..y1 {
            for x in x0..x1 {
                let texel = atlas.pages[page as usize][(y * atlas.page_size + x) as usize];
                assert_eq!(texel, m.texels[0], "map {i} at ({x}, {y})");
            }
        }
    }
}

#[test]
fn cells_never_overlap_and_maps_at_the_cap_pair_up() {
    let cap = max_map_edge(MAX_PAGE);
    let maps: Vec<Lightmap> = (0..5).map(|i| map(i, cap, i as f32)).collect();
    let atlas = pack(&maps);
    assert_eq!(atlas.page_size, MAX_PAGE);
    assert_eq!(atlas.pages.len(), 2, "four at the cap to a page, then one");
    let cells: Vec<_> = (0..maps.len()).map(|i| cell(&atlas, i, cap)).collect();
    for (i, a) in cells.iter().enumerate() {
        assert!(a.3 <= MAX_PAGE && a.4 <= MAX_PAGE, "inside the page");
        for b in &cells[i + 1..] {
            let apart = a.0 != b.0 || a.3 <= b.1 || b.3 <= a.1 || a.4 <= b.2 || b.4 <= a.2;
            assert!(apart, "{a:?} overlaps {b:?}");
        }
    }
    let half = max_map_edge(1024);
    let at_half: Vec<Lightmap> = (0..2).map(|i| map(i, half, 1.0)).collect();
    let atlas = pack(&at_half);
    assert_eq!(
        (atlas.page_size, atlas.pages.len()),
        (1024, 1),
        "two per 1024 page"
    );
}

/// A deterministic spread of map sizes like a game level's: 28 at the default cap
/// and 400 small to mid ones.
fn level_sizes() -> Vec<u32> {
    let mut state = 0x9e37_79b9_u32;
    let mut sizes = vec![max_map_edge(1024); 28];
    sizes.extend((0..400).map(|_| {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        16 + (state >> 8) % 240
    }));
    sizes
}

#[test]
fn a_level_fills_its_pages_before_opening_more() {
    let sizes = level_sizes();
    let maps: Vec<Lightmap> = sizes.iter().map(|&s| map(0, s, 1.0)).collect();
    let atlas = pack(&maps);
    let area: u64 = sizes.iter().map(|&s| u64::from(cell_edge(s)).pow(2)).sum();
    let page = u64::from(atlas.page_size).pow(2);
    let least = area.div_ceil(page) as usize;
    assert!(
        atlas.pages.len() <= least + 1,
        "{} pages for {least} pages' worth of cells",
        atlas.pages.len()
    );
}

#[test]
fn packing_is_deterministic_and_empty_is_empty() {
    let maps = [map(4, 12, 0.5), map(9, 30, 0.25)];
    assert_eq!(pack(&maps), pack(&maps));
    let none = pack(&[]);
    assert!(none.pages.is_empty() && none.placements.is_empty());
    let one = pack(&[map(1, 1, 2.0)]);
    assert_eq!((one.page_size, one.pages.len()), (MIN_PAGE, 1));
}
