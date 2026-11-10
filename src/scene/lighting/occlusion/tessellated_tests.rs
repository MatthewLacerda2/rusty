//! A level built of tessellated meshes bakes about as fast as the same level of
//! boxes (#1981): a wall whose triangles no two make a quad fuses into a sheet
//! all the same, so its rays are proved stopped instead of cast. Counted in
//! rays, not timed (#1883), so it holds on any machine.

use super::bench_town::{town, Make};
use super::gather_occlusion;
use super::sample::Baking;
use super::Grid;

/// Rays cast baking `cells` cells spread over `make`'s town, per cell.
fn rays_per_cell(make: Make, cells: u64) -> u64 {
    let input = gather_occlusion(&town(3, 0.25, 2, make));
    let scene = Baking::of(input.objects());
    let (min, max) = scene.bounds.expect("the town has bounds");
    let grid = Grid::over(min, max).expect("a grid");
    let count = grid.cell_count() as u64;
    for k in 0..cells {
        let cell = k.wrapping_mul(0x9E37_79B9_7F4A_7C15) % count;
        std::hint::black_box(scene.cell(&grid, cell as usize));
    }
    scene.rays_cast() / cells
}

#[test]
fn a_tessellated_town_casts_about_the_rays_a_box_built_one_does() {
    let boxes = rays_per_cell(Make::Boxes, 60);
    let authored = rays_per_cell(Make::Authored, 60);
    // Before its faces fused, the tessellated town cast some 18 times as many.
    assert!(
        authored <= boxes * 2,
        "{authored} rays a cell against the boxes' {boxes}"
    );
}
