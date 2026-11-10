//! A level built of tessellated meshes bakes about as fast as the same level of
//! boxes (#1981): a wall whose triangles no two make a quad fuses into a sheet
//! all the same, so its rays are proved stopped instead of cast. Counted in
//! rays, not timed (#1883), so it holds on any machine.

use glam::Vec3;

use super::bench::shift;
use super::bench::town::{town, Make};
use super::gather_occlusion;

/// Rays cast baking `cells` cells spread over `make`'s town, per cell. The
/// town is moved and scaled off round coordinates, as an imported level's
/// corners are, so faces join only where they share a corner bit for bit.
fn rays_per_cell(make: Make, cells: u64) -> u64 {
    let mut level = town(3, 0.25, 2, make);
    shift(&mut level, Vec3::new(0.137, 0.053, -0.211), 1.07);
    let input = gather_occlusion(&level);
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
