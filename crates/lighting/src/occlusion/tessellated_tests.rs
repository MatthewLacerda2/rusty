use crate::occlusion::bench::town::{town, Make};
use crate::occlusion::gather_occlusion;

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
    assert!(
        authored <= boxes * 2,
        "{authored} rays a cell against the boxes' {boxes}"
    );
}
