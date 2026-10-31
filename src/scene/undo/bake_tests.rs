//! A lighting bake is no edit (#1631): Undo never takes a bake back, and a bake
//! landing between edits never joins the step before it.

use glam::Vec3;

use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::lighting::lightmap::LightmapEntry;
use crate::scene::{Scene, Sh9};

/// A scene with one crate, two light probes and a reflection probe, unbaked.
fn scene_with_probes() -> (Scene, u32) {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Crate", Some(Primitive::Box));
    scene.probes.add_probe(Vec3::ZERO);
    scene.probes.add_probe(Vec3::X);
    scene.reflection_probes.add(Vec3::ZERO, Vec3::ONE);
    (scene, id)
}

/// What Generate Lighting writes, outside any edit.
fn bake(scene: &mut Scene, id: u32) {
    let mut sh = Sh9::zero();
    sh.coeffs[0] = [1.0, 0.5, 0.25];
    for probe in &mut scene.probes.probes {
        probe.sh = sh;
    }
    scene
        .reflection_probes
        .set_cubemap(0, "probe_0.ktx2".into());
    scene.lightmaps.pages = vec!["lightmap_0.png".into()];
    scene.lightmaps.entries = vec![LightmapEntry {
        entity: id,
        page: 0,
        scale_offset: [1.0, 1.0, 0.0, 0.0],
    }];
}

fn assert_baked(scene: &Scene, id: u32) {
    assert!(scene.probes.probes.iter().all(|p| p.sh.coeffs[0][0] == 1.0));
    assert_eq!(
        scene.reflection_probes.probes[0].cubemap_path,
        "probe_0.ktx2"
    );
    assert_eq!(scene.lightmaps.get(id).map(|e| e.page), Some(0));
}

#[test]
fn undoing_a_setting_or_a_probe_move_after_a_bake_keeps_the_bake() {
    let (mut scene, id) = scene_with_probes();
    scene.begin_edits();
    scene.ambient_intensity = 0.25;
    scene.commit_edits(None);
    scene.begin_edits();
    scene.probes.move_probe(1, Vec3::Y);
    scene.commit_edits(None);
    bake(&mut scene, id);
    scene.undo();
    assert_eq!(
        scene.probes.probes[1].position,
        Vec3::X,
        "the move is undone"
    );
    scene.undo();
    assert_ne!(scene.ambient_intensity, 0.25, "the setting is undone");
    assert_baked(&scene, id);
    scene.redo();
    scene.redo();
    assert_eq!(scene.probes.probes[1].position, Vec3::Y);
    assert_baked(&scene, id);
}

#[test]
fn a_change_landing_between_edits_never_joins_the_last_step() {
    let (mut scene, id) = scene_with_probes();
    scene.begin_edits();
    scene.world.set_name(id, "Renamed".into());
    scene.commit_edits(None);
    // A bake, and a scene-level write from outside the editor's edits.
    bake(&mut scene, id);
    scene.skybox_path = "watched.hdr".into();
    // An idle frame with the crate selected: the Inspector borrows it mutably.
    scene.begin_edits();
    drop(scene.world.transform_mut(id));
    scene.pause_edits();
    assert!(!scene.commit_follow_up(), "nothing was edited");
    scene.undo();
    assert_eq!(scene.skybox_path, "watched.hdr", "not part of the rename");
    assert_baked(&scene, id);
    assert_eq!(scene.undo(), None);
}

#[test]
fn a_bake_landing_on_an_idle_frame_neither_joins_the_last_step_nor_clears_redo() {
    let (mut scene, id) = scene_with_probes();
    scene.begin_edits();
    scene.world.set_name(id, "Renamed".into());
    scene.commit_edits(None);
    // Generate Lighting lands on a frame nobody touched, auto-placing a probe.
    scene.begin_edits();
    scene.unrecorded(|s| {
        s.probes.add_probe(Vec3::Y);
        bake(s, id);
    });
    scene.pause_edits();
    assert!(!scene.commit_follow_up(), "the bake is no edit");
    assert_eq!(scene.undo().as_deref(), Some("Edit Renamed"));
    assert_eq!(scene.probes.probes.len(), 3, "the bake's probe stays");
    assert_baked(&scene, id);
    // An idle frame's own follow-up after an Undo keeps it redoable.
    scene.begin_edits();
    scene.ambient_intensity = 0.5;
    scene.pause_edits();
    assert!(scene.commit_follow_up());
    assert_eq!(scene.redo_name(), Some("Edit Renamed"));
}

#[test]
fn a_bake_landing_mid_drag_splits_the_drag_and_joins_neither_part() {
    let (mut scene, _) = scene_with_probes();
    let start = scene.ambient_intensity;
    scene.begin_edits();
    scene.ambient_intensity = 0.25;
    scene.unrecorded(|s| s.probes.add_probe(Vec3::Z));
    scene.ambient_intensity = 0.5;
    scene.commit_edits(None);
    scene.undo();
    assert_eq!(scene.ambient_intensity, 0.25);
    scene.undo();
    assert_eq!(scene.ambient_intensity, start);
    assert_eq!(scene.probes.probes.len(), 3, "the bake's probe stays");
    assert_eq!(scene.undo(), None);
}
