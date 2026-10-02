//! Off-mesh links (#462) on the CS:GO-sized level: the bake generates drops off
//! the first floors and jumps onto the crates, a moved crate regenerates only the
//! links near it, matching a full bake, and a path from a roof edge to the street
//! takes a drop. Ignored by default: bake timings with links on, for the PR.

use std::time::Instant;

use glam::Vec3;
use rusty::navigation::{NavPathStatus, NavigationGraph, OffMeshLink, OffMeshLinkKind, Rebake};
use rusty::scene::Scene;

use super::{add_box, bake, level};

/// The level with drops (to 4.5 m), jumps up crates (1.2 m) and 2 m gap jumps on.
fn linked_level(spacing: f32) -> (Scene, u32) {
    let mut scene = level::build();
    scene.nav_settings.grid_spacing = spacing;
    scene.nav_settings.drop_height = 4.5;
    scene.nav_settings.jump_height = 1.2;
    scene.nav_settings.jump_distance = 2.0;
    let crate_id = add_box(
        &mut scene,
        Vec3::new(100.0, 0.0, 30.0),
        Vec3::new(101.0, 1.0, 31.0),
    );
    (scene, crate_id)
}

fn links(g: &NavigationGraph) -> Vec<OffMeshLink> {
    g.offmesh_links().cloned().collect()
}

fn move_to(scene: &mut Scene, id: u32, position: Vec3) {
    if let Some(mut t) = scene.world.transform_mut(id) {
        t.position = position;
    }
    scene.update_entity_collider(id);
}

#[test]
fn the_level_gets_drops_and_jumps_and_a_moved_crate_matches_a_full_bake() {
    let (mut scene, crate_id) = linked_level(1.0);
    let mut g = bake(&mut scene, level::BOUNDS);
    let all = links(&g);
    let count = |k| all.iter().filter(|l| l.kind == k).count();
    assert!(
        count(OffMeshLinkKind::Drop) > 50,
        "{}",
        count(OffMeshLinkKind::Drop)
    );
    assert!(
        count(OffMeshLinkKind::Jump) > 20,
        "{}",
        count(OffMeshLinkKind::Jump)
    );
    move_to(&mut scene, crate_id, Vec3::new(104.5, 0.5, 32.5));
    assert!(matches!(g.sync(&scene), Rebake::Incremental(_)));
    assert_eq!(links(&g), links(&bake(&mut scene, level::BOUNDS)));
}

#[test]
fn a_drop_off_the_first_floor_is_a_shortcut_to_the_street() {
    let (mut scene, _) = linked_level(1.0);
    let g = bake(&mut scene, level::BOUNDS);
    let drop = links(&g)
        .into_iter()
        .find(|l| l.kind == OffMeshLinkKind::Drop && l.start.y >= 3.9)
        .expect("a drop off a first floor");
    let path = g.calculate_path(drop.start, drop.end);
    assert_eq!(path.status, NavPathStatus::Complete);
    assert_eq!(path.links.len(), 1, "straight off the edge");
    assert_eq!(path.links[0].1.kind, OffMeshLinkKind::Drop);
}

/// `cargo test --release --features dev --test integration -- --ignored --nocapture measure_links`
#[test]
#[ignore = "measurement for the PR record; run in release"]
fn measure_links_bake_and_rebake() {
    for spacing in [1.0, 0.5, 0.25] {
        let (mut scene, crate_id) = linked_level(spacing);
        let mut g = bake(&mut scene, level::BOUNDS);
        let ms = |start: Instant| start.elapsed().as_secs_f64() * 1e3;
        let start = Instant::now();
        g.bake(&scene);
        let full = ms(start);
        let generated = g.offmesh_links().count();
        move_to(&mut scene, crate_id, Vec3::new(104.5, 0.5, 32.5));
        let start = Instant::now();
        g.sync(&scene);
        let moved = ms(start);
        scene.nav_settings.drop_height = 0.0;
        scene.nav_settings.jump_height = 0.0;
        scene.nav_settings.jump_distance = 0.0;
        let start = Instant::now();
        g.bake(&scene);
        let without = ms(start);
        eprintln!(
            "spacing {spacing}: {generated} links; full bake {full:.2} ms ({without:.2} ms \
             without links); moved crate {moved:.3} ms"
        );
    }
}
