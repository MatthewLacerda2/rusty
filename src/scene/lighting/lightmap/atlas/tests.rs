//! The atlas packer (#438, #1694): every lightmap lands whole, inside one page, in
//! a cell no other overlaps, with its texels where its scale/offset says and its
//! edge extended over its cell; the smallest page that fits is chosen, up to the
//! Max Lightmap Size; a map at the cap is its whole page, and one above half the
//! page leaves the rest to smaller maps (#1704); and pages are filled before new
//! ones open.

use glam::Vec3;

use super::*;

/// A `size`² lightmap for `entity`, every texel `value`.
fn map(entity: u32, size: u32, value: f32) -> Lightmap {
    Lightmap {
        masks: Vec::new(),
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
    let atlas = pack(&maps, MAX_PAGE);
    assert_eq!((atlas.page_size, atlas.pages.len()), (64, 1));
    for (i, m) in maps.iter().enumerate() {
        assert_eq!(atlas.placements[i].0, m.entity);
        // The whole cell, ring and slack included, holds the map's own light.
        let (page, x0, y0, x1, y1) = cell(&atlas, i, m.size);
        assert!(x0 % GRID == 0 && y0 % GRID == 0, "aligned to the grid");
        for y in y0..y1 {
            for x in x0..x1 {
                let texel = atlas.pages[page as usize][(y * atlas.page_size + x) as usize];
                assert_eq!(texel, m.texels[0], "map {i} at ({x}, {y})");
            }
        }
    }
}

/// Every placement's cell lies inside its page and overlaps no other.
fn assert_apart(atlas: &LightmapAtlas, sizes: &[u32]) {
    let cells: Vec<_> = (0..sizes.len()).map(|i| cell(atlas, i, sizes[i])).collect();
    for (i, a) in cells.iter().enumerate() {
        assert!(a.3 <= atlas.page_size && a.4 <= atlas.page_size, "inside");
        for b in &cells[i + 1..] {
            let apart = a.0 != b.0 || a.3 <= b.1 || b.3 <= a.1 || a.4 <= b.2 || b.4 <= a.2;
            assert!(apart, "{a:?} overlaps {b:?}");
        }
    }
}

#[test]
fn a_map_at_the_cap_is_its_whole_page_and_reads_back() {
    let cap = max_map_edge(256);
    assert_eq!(cell_edge(cap), 256, "the cap's cell is the page (#1704)");
    let maps: Vec<Lightmap> = (0..3).map(|i| map(i, cap, i as f32 + 1.0)).collect();
    let atlas = pack(&maps, 256);
    assert_eq!((atlas.page_size, atlas.pages.len()), (256, 3));
    for (i, m) in maps.iter().enumerate() {
        let (_, page, st) = atlas.placements[i];
        let (scale, at) = (cap as f32 / 256.0, PAD as f32 / 256.0);
        assert_eq!(st, [scale, scale, at, at], "map {i}'s scale/offset");
        let texels = &atlas.pages[page as usize];
        assert!(texels.iter().all(|&t| t == m.texels[0]), "map {i} fills it");
    }
    let wider = pack(&[map(7, 300, 1.0)], 256);
    assert_eq!(
        wider.placements[0].2[0],
        cap as f32 / 256.0,
        "cut to the cap"
    );
}

#[test]
fn small_maps_fill_the_page_around_one_above_half_the_edge() {
    let mut sizes = vec![700];
    sizes.extend([120; 20]);
    let maps: Vec<Lightmap> = sizes.iter().map(|&s| map(0, s, 1.0)).collect();
    let atlas = pack(&maps, 1024);
    assert_eq!((atlas.page_size, atlas.pages.len()), (1024, 1), "one page");
    assert_apart(&atlas, &sizes);
}

#[test]
fn the_page_never_exceeds_the_max_lightmap_size() {
    let sizes = [100; 5];
    let maps: Vec<Lightmap> = sizes.iter().map(|&s| map(0, s, 1.0)).collect();
    let atlas = pack(&maps, 128);
    assert_eq!((atlas.page_size, atlas.pages.len()), (128, 5));
    assert_apart(&atlas, &sizes);
    assert_eq!(pack(&maps, MAX_PAGE).pages.len(), 1, "512 holds them all");
    for (asked, held) in [
        (0, MIN_PAGE),
        (700, 1024),
        (4096, 4096),
        (u32::MAX, MAX_PAGE),
    ] {
        assert_eq!(page_edge(asked), held, "{asked}");
    }
}

/// A deterministic spread of map sizes like a game level's: 28 at the old 504
/// cap and 400 small to mid ones.
fn level_sizes() -> Vec<u32> {
    let mut state = 0x9e37_79b9_u32;
    let mut sizes = vec![max_map_edge(512); 28];
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
    let atlas = pack(&maps, 1024);
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
    assert_eq!(pack(&maps, 1024), pack(&maps, 1024));
    let none = pack(&[], 1024);
    assert!(none.pages.is_empty() && none.placements.is_empty());
    let one = pack(&[map(1, 1, 2.0)], 1024);
    assert_eq!((one.page_size, one.pages.len()), (MIN_PAGE, 1));
}
