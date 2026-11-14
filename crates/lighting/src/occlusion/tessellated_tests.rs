//! A level built of tessellated meshes bakes about as fast as the same level of
//! boxes (#1981): a wall whose triangles no two make a quad fuses into a sheet
//! all the same, so its rays are proved stopped instead of cast. Counted in
//! rays, not timed (#1883), so it holds on any machine.

use crate::occlusion::bench::town::{town, Make};
use crate::occlusion::gather_occlusion;

/// Rays cast baking `cells` cells spread over `make`'s town, per cell. The town
/// sits on round coordinates: moved off them, a tessellated wall fuses to the
/// texel at its edges where a box's own face proves exactly, and it casts about
/// twice its twin's rays (PR #1989).
fn rays_per_cell(make: Make, cells: u64) -> u64 {
    let input = gather_occlusion(&town(3, 0.25, 2, make));
    let sampler = input.sampler().expect("a grid");
    sampler.spread(cells);
    sampler.rays_cast() / cells
}

#[test]
fn a_tessellated_town_casts_about_the_rays_a_box_built_one_does() {
    let boxes = rays_per_cell(Make::Boxes, 60);
    let authored = rays_per_cell(Make::Authored, 60);
    // Before its faces fused, the tessellated town cast some 13 times as many.
    assert!(
        authored <= boxes * 2,
        "{authored} rays a cell against the boxes' {boxes}"
    );
}
