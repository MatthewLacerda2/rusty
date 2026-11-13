//! Incremental rebakes regenerate the links near a change and match a full bake.

use glam::Vec3;

use super::super::super::test_support::{add_box, move_to};
use super::super::super::Rebake;
use super::*;

/// The platform level plus crates by its +x ledge and the ladder link.
fn level(radius: f32, spacing: f32) -> (Scene, Vec<u32>) {
    let mut scene = platform();
    scene.nav_settings.agent_radius = radius;
    scene.nav_settings.grid_spacing = spacing;
    let mut ids = vec![
        add_box(
            &mut scene,
            Vec3::new(10.0, 0.0, 2.0),
            Vec3::new(11.0, 1.0, 3.0),
        ),
        add_box(
            &mut scene,
            Vec3::new(14.0, 0.0, 12.0),
            Vec3::new(16.0, 1.0, 14.0),
        ),
    ];
    let link = scene.add_entity("link".to_string());
    scene.world.transform_mut(link).unwrap().position = Vec3::new(12.0, 0.0, 17.0);
    scene
        .world
        .set_offmesh_link(link, Some(OffMeshLinkComponent::default()));
    ids.push(link);
    (scene, ids)
}

fn edits_match_a_full_bake(radius: f32, spacing: f32) {
    let (mut scene, ids) = level(radius, spacing);
    let mut g = baked(&scene);
    let edits: [&dyn Fn(&mut Scene); 4] = [
        &|s| move_to(s, ids[0], Vec3::new(9.5, 0.5, 5.5)), // onto the drop landing
        &|s| move_to(s, ids[1], Vec3::new(17.0, 0.5, 3.0)),
        &|s| {
            add_box(s, Vec3::new(8.0, 0.0, 1.0), Vec3::new(9.0, 2.0, 2.0));
        },
        &|s| move_to(s, ids[2], Vec3::new(4.0, 3.0, 4.0)), // the link up top
    ];
    for (i, edit) in edits.iter().enumerate() {
        let before = links(&g);
        edit(&mut scene);
        let outcome = g.sync(&scene);
        assert!(matches!(outcome, Rebake::Incremental(_)), "edit {i}");
        assert_eq!(links(&g), links(&baked(&scene)), "edit {i}");
        assert_ne!(links(&g), before, "edit {i} changed some link");
        let far = Vec3::new(4.0, 3.0, 2.0);
        let path = |g: &NavigationGraph| g.calculate_path(far, Vec3::new(18.0, 0.0, 18.0));
        assert_eq!(path(&g), path(&baked(&scene)), "edit {i}: same route");
    }
}

#[test]
fn incremental_links_match_a_full_bake() {
    edits_match_a_full_bake(0.5, 1.0);
}

#[test]
fn incremental_links_match_a_full_bake_on_a_fine_grid() {
    edits_match_a_full_bake(0.3, 0.5);
}

#[test]
fn a_link_edit_alone_rebakes_no_spans() {
    let (mut scene, ids) = level(0.5, 1.0);
    let mut g = baked(&scene);
    let spans = g.spans.clone();
    scene.world.offmesh_link_mut(ids[2]).unwrap().bidirectional = false;
    assert!(matches!(g.sync(&scene), Rebake::Incremental(_)));
    assert_eq!(g.spans, spans);
    assert_eq!(links(&g), links(&baked(&scene)));
    assert_eq!(g.sync(&scene), Rebake::Unchanged, "and then nothing to do");
}
