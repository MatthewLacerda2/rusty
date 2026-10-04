//! The atlas packer (#438): every lightmap lands whole, inside one page, with no two
//! overlapping, its texels where its scale/offset says, and the smallest page that
//! fits is chosen.

use glam::Vec3;

use super::*;

/// A `size`² lightmap for `entity`, every texel `value`.
fn map(entity: u32, size: u32, value: f32) -> Lightmap {
    Lightmap {
        entity,
        size,
        texels: vec![Vec3::splat(value); (size * size) as usize],
    }
}

/// The page texel a lightmap UV lands on, through a placement's scale/offset.
fn lookup(atlas: &LightmapAtlas, placement: usize, uv: [f32; 2]) -> Vec3 {
    let (_, page, st) = atlas.placements[placement];
    let edge = atlas.page_size as f32;
    let x = ((uv[0] * st[0] + st[2]) * edge) as usize;
    let y = ((uv[1] * st[1] + st[3]) * edge) as usize;
    atlas.pages[page as usize][y * atlas.page_size as usize + x]
}

#[test]
fn small_bakes_share_the_smallest_page_that_fits() {
    let maps = [map(1, 16, 1.0), map(2, 8, 2.0), map(3, 16, 3.0)];
    let atlas = pack(&maps);
    assert_eq!((atlas.page_size, atlas.pages.len()), (64, 1));
    for (i, m) in maps.iter().enumerate() {
        assert_eq!(atlas.placements[i].0, m.entity);
        for uv in [[0.01, 0.01], [0.5, 0.5], [0.99, 0.99]] {
            assert_eq!(lookup(&atlas, i, uv), m.texels[0], "map {i} at {uv:?}");
        }
    }
}

#[test]
fn rects_never_overlap_and_big_bakes_span_pages() {
    let maps: Vec<Lightmap> = (0..5).map(|i| map(i, 600, i as f32)).collect();
    let atlas = pack(&maps);
    assert_eq!(atlas.page_size, MAX_PAGE);
    assert_eq!(
        atlas.pages.len(),
        5,
        "a 600² lightmap leaves no room beside it"
    );
    let rect =
        |(_, page, st): (u32, u32, [f32; 4])| (page, st[2], st[3], st[2] + st[0], st[3] + st[1]);
    let rects: Vec<_> = atlas.placements.iter().copied().map(rect).collect();
    for (i, a) in rects.iter().enumerate() {
        assert!(a.3 <= 1.0 && a.4 <= 1.0, "inside the page");
        for b in &rects[i + 1..] {
            let apart = a.0 != b.0 || a.3 <= b.1 || b.3 <= a.1 || a.4 <= b.2 || b.4 <= a.2;
            assert!(apart, "{a:?} overlaps {b:?}");
        }
    }
}

#[test]
fn packing_is_deterministic_and_empty_is_empty() {
    let maps = [map(4, 12, 0.5), map(9, 30, 0.25)];
    assert_eq!(pack(&maps), pack(&maps));
    let none = pack(&[]);
    assert!(none.pages.is_empty() && none.placements.is_empty());
}
