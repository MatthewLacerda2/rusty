//! A page's mip chain (#1694): the levels the atlas keeps apart never mix two maps
//! or a map's empty background, the bake fills a map's background from its charts,
//! and mixed directions average to a less directional texel.

use glam::Vec3;

use super::atlas::{pack, MIP_LEVELS, PAD};
use super::bake::Lightmap;
use super::pages::{level_count, levels};
use super::raster::fill_rest;

fn map(entity: u32, size: u32, value: f32) -> Lightmap {
    Lightmap {
        entity,
        size,
        texels: vec![Vec3::splat(value); (size * size) as usize],
        directions: Vec::new(),
    }
}

#[test]
fn every_kept_level_of_a_map_reads_only_its_own_light() {
    let maps = [map(1, 13, 1.0), map(2, 7, 5.0), map(3, 21, 0.25)];
    let atlas = pack(&maps);
    let edge = atlas.page_size;
    assert_eq!(level_count(edge), MIP_LEVELS);
    let chain = levels(&atlas.pages[0], edge);
    for (i, m) in maps.iter().enumerate() {
        let st = atlas.placements[i].2;
        for (l, level) in chain.iter().enumerate() {
            let (side, scale) = (edge >> l, 1u32 << l);
            // The map's texels at this level, and one more texel all round: what
            // bilinear filtering at its border reads.
            let x0 = (st[2] * edge as f32) as u32 / scale - 1;
            let y0 = (st[3] * edge as f32) as u32 / scale - 1;
            let x1 = ((st[2] * edge as f32) as u32 + m.size).div_ceil(scale) + 1;
            let y1 = ((st[3] * edge as f32) as u32 + m.size).div_ceil(scale) + 1;
            for y in y0..y1 {
                for x in x0..x1 {
                    let texel = level[(y * side + x) as usize];
                    assert_eq!(texel, m.texels[0], "map {i}, level {l}, ({x}, {y})");
                }
            }
        }
    }
    assert!(
        PAD >= 1 << (MIP_LEVELS - 1),
        "the ring covers the last level"
    );
}

#[test]
fn a_map_s_background_is_filled_from_its_nearest_chart() {
    let mut texels = vec![Vec3::ZERO; 16];
    let mut filled = vec![false; 16];
    (texels[0], filled[0]) = (Vec3::X, true);
    (texels[15], filled[15]) = (Vec3::Y, true);
    fill_rest(&mut texels, &filled, 4);
    assert_eq!((texels[1], texels[14]), (Vec3::X, Vec3::Y));
    assert!(
        texels.iter().all(|t| *t != Vec3::ZERO),
        "nothing left black"
    );
    let mut none = vec![Vec3::ZERO; 4];
    fill_rest(&mut none, &[false; 4], 2);
    assert_eq!(none, [Vec3::ZERO; 4], "no chart: nothing to fill from");
}

#[test]
fn mixed_directions_average_to_a_weaker_one() {
    assert_eq!(levels(&[Vec3::X; 16], 4).len(), 1, "a 4² page: no mips");
    let page: Vec<Vec3> = (0..64)
        .map(|i| match i % 2 {
            0 => Vec3::X,
            _ => Vec3::new(0.0, 0.6, 0.0),
        })
        .collect();
    let chain = levels(&page, 8);
    assert_eq!(chain.len(), 2, "8² and 4²");
    let mixed = chain[1][0];
    assert!((mixed - Vec3::new(0.5, 0.3, 0.0)).length() < 1e-6);
    assert!(mixed.length() < 0.6, "less directional than either");
}
