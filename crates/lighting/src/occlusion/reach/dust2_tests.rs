use glam::{Vec2, Vec3};

use crate::occlusion::Reach;
use rusty_scene::lighting::floor::{FloorCell, FloorGrid};
use rusty_scene::lighting::occlusion::{FloorSource, Grid, CELL_SIZE, MAX_CELLS, MAX_GRID_CELLS};
use rusty_scene::Scene;

fn lanes() -> FloorGrid {
    let lane = |at: usize| (at % 50) >= 22 && (at % 50) < 28;
    let open = FloorCell {
        floor: 0.0,
        ceiling: f32::INFINITY,
    };
    let cells = (0..200 * 200).map(|i| (lane(i % 200) || lane(i / 200)).then_some(open));
    FloorGrid {
        origin: Vec2::splat(-99.5),
        cell: 1.0,
        width: 200,
        depth: 200,
        cells: cells.collect(),
        upper: Vec::new(),
        inset: 0.0,
    }
}

#[test]
fn a_dust2_sized_level_with_a_floor_takes_two_metre_cells_within_both_caps() {
    let (lo, hi) = (Vec3::new(-100.0, 0.0, -100.0), Vec3::new(100.0, 8.0, 100.0));
    let every = Grid::over(lo, hi).expect("culled");
    assert!(every.cell > CELL_SIZE, "every cell sampled: {every:?}");
    let reach = Reach::over(&Scene::new(), &lanes());
    let cells = reach.grid(lo, hi).expect("culled");
    assert_eq!(cells.source, FloorSource::Navmesh);
    assert_eq!(cells.grid.cell, CELL_SIZE, "{:?}", cells.grid);
    assert!(
        cells.grid.cell_count() <= MAX_GRID_CELLS,
        "{}",
        cells.grid.cell_count()
    );
    let sampled = cells.sampled_count();
    assert!(sampled <= MAX_CELLS, "{sampled} sampled");
    let share = reach.coverage((lo, hi)).expect("a floor");
    assert!((0.15..0.3).contains(&share), "{share}");
}
