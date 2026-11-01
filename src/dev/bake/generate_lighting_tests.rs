//! Generate Lighting (#832): both bakes in order, with the scene's settings, and the
//! result in the scene file. The probe step skips without a GPU adapter, so the
//! probe-bake flags are not asserted; the bakes themselves are tested beside them.

use glam::Vec3;

use super::{generate_lighting, GenerateReport, UNSAVED};
use crate::components::{ColliderComponent, ColliderShape};
use crate::dev::bake::progress::{BakeLog, Line};
use crate::scene::authoring::{primitive_mesh_component, Primitive};
use crate::scene::{LightingSave, Scene};

/// A saved scene with one static, lightmappable floor and cheap bake settings.
fn saved_floor(name: &str) -> (Scene, String) {
    let dir = crate::test_temp::dir().join(format!("generate_{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let mut scene = Scene::new();
    let floor = scene.add_entity("Floor".into());
    scene
        .world
        .set_mesh(floor, primitive_mesh_component(Primitive::Plane));
    scene.world.set_static(floor, true);
    // A collider gives probe placement static bounds to fill.
    let shape = ColliderShape::Box {
        size: Vec3::new(10.0, 4.0, 10.0),
    };
    let collider = ColliderComponent {
        shape,
        ..crate::scene::authoring::default_collider()
    };
    scene.world.set_collider(floor, Some(collider));
    let s = &mut scene.lighting_settings.lightmaps;
    (s.texels_per_unit, s.samples, s.bounces, s.max_resolution) = (2.0, 4, 1, 16);
    scene.save_to_file(&path).unwrap();
    (scene, path)
}

#[test]
fn gpu_generate_bakes_lightmaps_then_probes_and_writes_the_scene() {
    let (mut scene, path) = saved_floor("writes");
    let (tick, lines) = (std::cell::Cell::new(0), std::cell::RefCell::new(Vec::new()));
    let clock = || std::time::Duration::from_secs(tick.replace(tick.get() + 1));
    let mut log = BakeLog::new(clock, |l: Line| lines.borrow_mut().push(l.text));
    let report = generate_lighting(&mut scene, Some(&path), None, &mut |_| {}, &mut log);
    drop(log);
    let report = report.unwrap();
    // The log heard the lightmap step first and the total last (#1667).
    let lines = lines.into_inner();
    assert!(lines[0].starts_with("[Lighting] lightmaps"), "{lines:?}");
    assert!(lines.last().unwrap().starts_with("[Lighting] done in"));
    assert_eq!(report.lightmaps, 1);
    assert!(report.probes.auto_placed_light, "the probe step ran after");
    assert_eq!(report.saved, LightingSave::Written);

    let mut back = Scene::new();
    back.load_from_file(&path).unwrap();
    assert_eq!(back.lightmaps, scene.lightmaps, "no re-save needed");
    assert_eq!(back.probes.probes.len(), scene.probes.probes.len());
    assert_eq!(back.lighting_settings, scene.lighting_settings);
}

#[test]
fn gpu_generate_bakes_with_the_scenes_settings() {
    let (mut scene, path) = saved_floor("settings");
    scene.lighting_settings.lightmaps.directional = false;
    generate_lighting(
        &mut scene,
        Some(&path),
        None,
        &mut |_| {},
        &mut BakeLog::off(),
    )
    .unwrap();
    assert!(scene.lightmaps.directions.is_empty(), "directional was off");
}

#[test]
fn an_unsaved_scene_cannot_generate() {
    let mut scene = Scene::new();
    assert_eq!(
        generate_lighting(&mut scene, None, None, &mut |_| {}, &mut BakeLog::off()),
        Err(UNSAVED.to_string())
    );
}

#[test]
fn the_summary_says_whether_the_scene_needs_saving() {
    let mut report = GenerateReport {
        lightmaps: 2,
        probes: Default::default(),
        saved: LightingSave::Written,
    };
    assert_eq!(
        report.summary(),
        "Generated lighting: 2 lightmap(s), 0 light probe(s), 0 reflection probe(s); saved"
    );
    report.saved = LightingSave::NeedsSceneSave;
    assert!(report.summary().ends_with("; save the scene to keep it"));
}

/// Generate Lighting writes the scene file, so Play refuses it as it refuses Save
/// (#1340), before any bake runs.
#[test]
fn generate_is_refused_in_play() {
    let (mut scene, path) = saved_floor("play");
    let before = std::fs::read(&path).unwrap();
    scene.set_playing(true);
    let refused = generate_lighting(
        &mut scene,
        Some(&path),
        None,
        &mut |_| {},
        &mut BakeLog::off(),
    );
    assert_eq!(refused.unwrap_err(), crate::scene::LIGHTING_IN_PLAY);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "the file is untouched"
    );
}

/// One bake at a time (#1602): while the editor's Generate Lighting runs, a script's
/// `Generate` and `Bake` are refused before they place or bake anything.
#[test]
fn a_script_bake_is_refused_while_the_editors_runs() {
    let (mut scene, path) = saved_floor("running");
    scene.lighting_bake_running = true;
    let refused = generate_lighting(
        &mut scene,
        Some(&path),
        None,
        &mut |_| {},
        &mut BakeLog::off(),
    );
    assert_eq!(refused.unwrap_err(), super::BAKE_RUNNING);
    let params = super::super::lighting::LightingBakeParams::default();
    let bake = super::super::lighting::bake_lighting(
        &mut scene,
        Some(&path),
        None,
        params,
        &mut |_| {},
        &mut BakeLog::off(),
    );
    assert_eq!(bake.unwrap_err(), super::BAKE_RUNNING);
    assert!(scene.probes.is_empty(), "nothing was placed");
}

/// A bake inside a console line's step (#1631): the probes it places and the
/// lightmaps it applies are no edit, so undoing the line keeps them.
#[test]
fn gpu_generate_inside_an_edit_is_no_part_of_its_step() {
    let (mut scene, path) = saved_floor("undo_1631");
    scene.begin_edits();
    scene.ambient_intensity = 0.25;
    generate_lighting(
        &mut scene,
        Some(&path),
        None,
        &mut |_| {},
        &mut BakeLog::off(),
    )
    .unwrap();
    scene.commit_edits(None);
    let (probes, lightmaps) = (scene.probes.probes.len(), scene.lightmaps.clone());
    assert!(probes > 0, "the bake placed probes");
    while scene.undo().is_some() {}
    assert_eq!(scene.probes.probes.len(), probes);
    assert_eq!(scene.lightmaps, lightmaps);
}
