use glam::Vec3;

use super::tests::{bake_reaching, floor, hangar, LAMP};
use crate::occlusion::{is_current, Reach};
use rusty_scene::Scene;

fn lamp(scene: &Scene) -> u32 {
    scene
        .world
        .ids_with_light()
        .into_iter()
        .next()
        .expect("the lamp")
}

#[test]
fn a_light_moved_within_its_cell_keeps_and_to_another_cell_re_bakes() {
    let scene = hangar(LAMP);
    let (held, _) = bake_reaching(&scene, Reach::over(&scene, &floor(&scene)));
    assert!(is_current(
        &scene,
        &held,
        &Reach::over(&scene, &floor(&scene))
    ));
    let cell = held.grid.index_of(LAMP).expect("in the grid");
    let near = LAMP + Vec3::splat(0.1);
    assert_eq!(held.grid.index_of(near), Some(cell), "the same cell");
    let mut nudged = hangar(LAMP);
    nudged
        .world
        .transform_mut(lamp(&nudged))
        .expect("lamp")
        .position = near;
    assert!(is_current(
        &nudged,
        &held,
        &Reach::over(&nudged, &floor(&nudged))
    ));
    let mut moved = hangar(LAMP);
    let far = Vec3::new(6.0, 8.5, -6.0);
    moved
        .world
        .transform_mut(lamp(&moved))
        .expect("lamp")
        .position = far;
    assert!(!is_current(
        &moved,
        &held,
        &Reach::over(&moved, &floor(&moved))
    ));
}

#[test]
fn a_floor_that_changes_the_reachable_cells_re_bakes() {
    let scene = hangar(LAMP);
    let plan = floor(&scene);
    let (held, _) = bake_reaching(&scene, Reach::over(&scene, &plan));
    assert!(is_current(&scene, &held, &Reach::over(&scene, &plan)));
    let mut ledge = plan.clone();
    let open = ledge.cells.iter().position(Option::is_some).expect("open");
    ledge.upper.push((open, 6.0));
    assert!(
        !is_current(&scene, &held, &Reach::over(&scene, &ledge)),
        "a ledge up high"
    );
    assert!(
        !is_current(&scene, &held, &Reach::everywhere()),
        "every cell sampled"
    );
}
