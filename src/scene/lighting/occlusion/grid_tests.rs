//! The PVS's cells (#1714): a margin round the bounds, a coarser cell past the cap,
//! and a lookup that agrees with each cell's box.

use glam::{UVec3, Vec3};

use super::{Grid, CELL_SIZE, MAX_CELLS};

#[test]
fn a_small_level_takes_two_metre_cells_and_a_margin() {
    let grid = Grid::over(Vec3::ZERO, Vec3::new(10.0, 3.0, 4.0));
    assert_eq!(grid.cell, CELL_SIZE);
    assert_eq!(grid.dims, UVec3::new(7, 4, 4));
    assert_eq!(
        grid.index_of(Vec3::new(-1.5, 0.0, 0.0)),
        Some(grid.index(UVec3::new(0, 1, 1)))
    );
    assert_eq!(
        grid.index_of(Vec3::new(-2.5, 0.0, 0.0)),
        None,
        "past the margin"
    );
    assert_eq!(grid.index_of(Vec3::NAN), None);
}

#[test]
fn a_dust2_sized_level_is_cut_into_at_most_the_cap() {
    let grid = Grid::over(
        Vec3::new(-100.0, -10.0, -100.0),
        Vec3::new(100.0, 20.0, 100.0),
    );
    assert!(grid.cell_count() <= MAX_CELLS, "{grid:?}");
    assert!(grid.cell > CELL_SIZE);
    let flat = Grid::over(Vec3::ZERO, Vec3::ZERO);
    assert_eq!(
        flat.dims,
        UVec3::splat(3),
        "a point still gets a cell and its margin"
    );
}

#[test]
fn every_cell_holds_its_own_centre_and_has_its_neighbours() {
    let grid = Grid::over(Vec3::ZERO, Vec3::new(6.0, 2.0, 6.0));
    for index in 0..grid.cell_count() {
        let (min, max) = grid.bounds(index);
        assert_eq!(grid.index_of((min + max) * 0.5), Some(index));
        assert_eq!(grid.coords(index).as_vec3() * grid.cell + grid.origin, min);
        let around: Vec<usize> = grid.neighbourhood(index).collect();
        assert!(around.contains(&index) && around.len() >= 8 && around.len() <= 27);
    }
}
