use glam::Vec2;

use super::fixtures::{bake_all, cell, cube, l_shape};
use super::{MeshMap, MARGIN};
use crate::lightmap::rasterize_claims;

const S: u32 = 96;

/// How much of texel `i` the cube's islands (its six cells) hold: `(centre inside, any overlap)`.
fn on_a_cell(i: usize) -> (bool, bool) {
    let texel = Vec2::new((i % S as usize) as f32, (i / S as usize) as f32);
    let (mut centre, mut overlap) = (false, false);
    for (lo, hi) in (0..6)
        .map(cell)
        .map(|(lo, hi)| (lo * S as f32, hi * S as f32))
    {
        let c = texel + 0.5;
        centre |= c.cmpgt(lo).all() && c.cmplt(hi).all();
        overlap |= (texel + 1.0).cmpgt(lo).all() && texel.cmplt(hi).all();
    }
    (centre, overlap)
}

#[test]
fn coverage_is_one_on_exactly_the_rasterised_texels_and_zero_on_the_margin() {
    let mesh = cube();
    let maps = bake_all(&mesh, &[MeshMap::Coverage, MeshMap::WorldNormal], S).unwrap();
    let (coverage, normal) = (maps[0].1.pixels(), maps[1].1.pixels());
    let raster: Vec<usize> = rasterize_claims(&mesh.surface(), S, S, &mut |_| {})
        .iter()
        .map(|p| p.at())
        .collect();
    let mut margin = 0;
    for (i, px) in coverage.iter().enumerate() {
        let covered = raster.contains(&i);
        let (centre, overlap) = on_a_cell(i);
        assert!(covered || !centre, "texel {i}: its centre is on an island");
        assert!(overlap || !covered, "texel {i} is off every island");
        let want = if covered {
            [1.0; 4]
        } else {
            [0.0, 0.0, 0.0, 1.0]
        };
        assert_eq!(*px, want, "texel {i}");
        margin += usize::from(!covered && normal[i] != [0.5, 0.5, 0.5, 1.0]);
    }
    assert!(
        margin > 0,
        "world_normal fills a margin that coverage leaves at 0"
    );
}

#[test]
fn coverage_has_no_margin_where_other_maps_grow_sixteen_texels() {
    let maps = bake_all(&l_shape(), &[MeshMap::Coverage, MeshMap::Position], S).unwrap();
    let (coverage, position) = (maps[0].1.pixels(), maps[1].1.pixels());
    let (_, hi) = cell(1);
    let y = (cell(1).0.y + hi.y) * 0.5 * S as f32;
    let row = y as usize * S as usize;
    let last = (hi.x * S as f32).floor() as usize - 1;
    assert_eq!(
        coverage[row + last][0],
        1.0,
        "the wall's last texel is covered"
    );
    for step in 1..=MARGIN as usize {
        let x = last + step;
        assert_eq!(coverage[row + x][0], 0.0, "{step} texels past the wall");
        assert!(position[row + x][3] == 1.0 && position[row + x][..3] != [0.0; 3]);
    }
}

#[test]
fn the_map_table_is_sorted_by_name_so_a_new_map_is_one_row_in_its_place() {
    let names = MeshMap::ALL.map(MeshMap::name);
    assert!(names.is_sorted(), "{names:?}");
    assert_eq!(MeshMap::parse("COVERAGE"), Ok(MeshMap::Coverage));
}
