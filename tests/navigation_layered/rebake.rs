//! The incremental rebake (#456) on the CS:GO-sized level: moving a crate and
//! dropping a carving obstacle rebakes a few cells, and the result is the navmesh
//! a full bake gives. Ignored by default: full vs incremental timings for the PR.

use std::time::Instant;

use glam::Vec3;
use rusty::components::NavMeshObstacleComponent;
use rusty::navigation::{NavigationGraph, Rebake};
use rusty::scene::Scene;

use super::{add_box, bake, level};

/// Move `id` to `position`, keeping its collider bounds current.
fn move_to(scene: &mut Scene, id: u32, position: Vec3) {
    if let Some(mut t) = scene.world.transform_mut(id) {
        t.position = position;
    }
    scene.update_entity_collider(id);
}

/// A carving 1 × 2 × 4 obstacle at `at`, carving at once.
fn add_carving_obstacle(scene: &mut Scene, at: Vec3) -> u32 {
    let id = scene.add_entity("door".to_string());
    if let Some(mut t) = scene.world.transform_mut(id) {
        t.position = at;
    }
    let o = NavMeshObstacleComponent {
        size: Vec3::new(1.0, 2.0, 4.0),
        carve: true,
        carve_only_stationary: false,
        ..Default::default()
    };
    scene.world.set_nav_obstacle(id, Some(o));
    id
}

fn assert_same(a: &NavigationGraph, b: &NavigationGraph) {
    assert_eq!(a.cell_start, b.cell_start);
    assert_eq!(a.spans, b.spans);
}

#[test]
fn level_edits_rebake_locally_and_match_a_full_bake() {
    let mut scene = level::build();
    let crate_id = add_box(
        &mut scene,
        Vec3::new(100.0, 0.0, 30.0),
        Vec3::new(101.0, 1.0, 31.0),
    );
    let mut g = bake(&mut scene, level::BOUNDS);
    move_to(&mut scene, crate_id, Vec3::new(104.5, 0.5, 32.5));
    let Rebake::Incremental(rects) = g.sync(&scene) else {
        panic!("a moved crate rebakes incrementally");
    };
    let cells: usize = rects.iter().map(|r| r.cells()).sum();
    assert!(cells < 200, "{cells} cells rebaked");
    add_carving_obstacle(&mut scene, Vec3::new(37.0, 1.0, 37.5));
    assert!(matches!(g.sync(&scene), Rebake::Incremental(_)));
    assert_same(&g, &bake(&mut scene, level::BOUNDS));
}

/// `cargo test --release --features dev --test integration -- --ignored --nocapture measure_rebake`
#[test]
#[ignore = "measurement for the PR record; run in release"]
fn measure_rebake_full_vs_incremental() {
    for spacing in [1.0, 0.5, 0.25] {
        let mut scene = level::build();
        scene.nav_settings.grid_spacing = spacing;
        let crate_id = add_box(
            &mut scene,
            Vec3::new(100.0, 0.0, 30.0),
            Vec3::new(101.0, 1.0, 31.0),
        );
        let mut g = bake(&mut scene, level::BOUNDS);
        let ms = |start: Instant| start.elapsed().as_secs_f64() * 1e3;
        let start = Instant::now();
        g.bake(&scene);
        let full = ms(start);
        let start = Instant::now();
        let unchanged = g.sync(&scene);
        let idle = ms(start);
        assert_eq!(unchanged, Rebake::Unchanged);
        move_to(&mut scene, crate_id, Vec3::new(104.5, 0.5, 32.5));
        let start = Instant::now();
        g.sync(&scene);
        let moved = ms(start);
        add_carving_obstacle(&mut scene, Vec3::new(37.0, 1.0, 37.5));
        let start = Instant::now();
        g.sync(&scene);
        let carved = ms(start);
        eprintln!(
            "spacing {spacing}: {} cells; full bake {full:.2} ms; unchanged sync {idle:.3} ms; \
             moved crate {moved:.3} ms; carving door {carved:.3} ms",
            g.width * g.height
        );
    }
}
