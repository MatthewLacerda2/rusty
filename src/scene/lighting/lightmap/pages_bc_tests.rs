//! A page's compressed levels (#1694): a BC6H block at mip level 2 covers 16
//! base texels, so the atlas aligns its cells to 16 (`atlas::GRID`). On a 4-texel
//! grid such a block held two maps, or a map and the page's empty black, and BC6H
//! bent a dim map's light by up to 8.6%; on the 16-texel one every map, at every
//! compressed level, reads its own light within BC6H's 5%.

use glam::Vec3;

use super::atlas::{pack, GRID, MAX_PAGE};
use super::bake::Lightmap;
use super::pages::colour_page;
use crate::asset::lightmap_page::decode_bc6h;

fn map(entity: u32, size: u32, light: Vec3) -> Lightmap {
    Lightmap {
        entity,
        size,
        texels: vec![light; (size * size) as usize],
        directions: Vec::new(),
    }
}

#[test]
fn every_compressed_level_of_a_map_reads_its_own_light() {
    // Contrasting light whose colours lie on no one line through black: the pairs
    // the 4-texel grid bent most (a blue-white map beside a dim red one).
    let (cool, dim) = (Vec3::new(0.5, 4.0, 16.0), Vec3::new(0.3, 0.02, 0.01));
    let (red, green) = (Vec3::new(2.0, 0.2, 0.2), Vec3::new(0.2, 2.0, 0.2));
    let maps = [
        map(1, 9, cool),
        map(2, 5, red),
        map(3, 1, dim),
        map(4, 2, green),
        map(5, 3, dim * 10.0),
    ];
    let atlas = pack(&maps, MAX_PAGE);
    let edge = atlas.page_size;
    let page = colour_page(&atlas.pages[0], edge, 2, &|| false).expect("never stopped");
    assert_eq!(page.levels.len(), 3, "the base and two mips");
    for l in 0..page.levels.len() {
        let (side, scale) = (edge >> l, 1u32 << l);
        let level = decode_bc6h(&page, l);
        for (i, m) in maps.iter().enumerate() {
            let [_, _, u, v] = atlas.placements[i].2;
            let (x0, y0) = ((u * edge as f32) as u32, (v * edge as f32) as u32);
            assert_eq!(
                (x0 % GRID, y0 % GRID),
                (4, 4),
                "map {i}: a cell on the grid"
            );
            for y in y0 / scale..(y0 + m.size).div_ceil(scale) {
                for x in x0 / scale..(x0 + m.size).div_ceil(scale) {
                    let (want, got) = (m.texels[0], level[(y * side + x) as usize]);
                    let err = (want - got).abs().max_element() / want.max_element();
                    assert!(
                        err <= 0.05,
                        "map {i}, level {l}, ({x}, {y}): {want} -> {got}"
                    );
                }
            }
        }
    }
}
