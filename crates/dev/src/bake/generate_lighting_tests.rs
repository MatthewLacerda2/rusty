use glam::Vec3;

use super::{generate_lighting, GenerateReport, UNSAVED};
use crate::bake::progress::{BakeLog, Line};
use rusty_components::{ColliderComponent, ColliderShape};
use rusty_edit::authoring::{primitive_mesh_component, Primitive};
use rusty_edit::LightingSave;
use rusty_scene::Scene;

fn saved_floor(name: &str) -> (Scene, String) {
    let dir = rusty_test_support::temp::dir().join(format!("generate_{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let mut scene = Scene::new();
    let floor = scene.add_entity("Floor".into());
    scene
        .world
        .set_mesh(floor, primitive_mesh_component(Primitive::Plane));
    scene.world.set_static(floor, true);
    let shape = ColliderShape::Box {
        size: Vec3::new(10.0, 4.0, 10.0),
    };
    let collider = ColliderComponent {
        shape,
        ..rusty_edit::authoring::default_collider()
    };
    scene.world.set_collider(floor, Some(collider));
    let s = &mut scene.lighting_settings.lightmaps;
    (s.texels_per_unit, s.samples, s.bounces, s.max_lightmap_size) = (2.0, 4, 1, 32);
    rusty_edit::save_to_file(&scene, &path).unwrap();
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
    let lines = lines.into_inner();
    assert!(
        lines[0].starts_with("[Lighting] steps: lightmaps →"),
        "{lines:?}"
    );
    assert!(
        lines[1].starts_with("[Lighting] 0:01 lightmaps"),
        "{lines:?}"
    );
    assert!(lines
        .iter()
        .any(|l| l.starts_with("[Lighting] occlusion took")));
    assert!(lines.last().unwrap().starts_with("[Lighting] done in"));
    assert_eq!(report.lightmaps, 1);
    assert!(report.probes.auto_placed_light, "the probe step ran after");
    assert_eq!(report.saved, LightingSave::Written);

    let mut back = Scene::new();
    rusty_edit::load_from_file(&mut back, &path).unwrap();
    assert_eq!(back.lightmaps, scene.lightmaps, "no re-save needed");
    assert_eq!(back.probes.probes().len(), scene.probes.probes().len());
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
        occlusion: Default::default(),
        occlusion_ms: 0,
        occlusion_kept: false,
        saved: LightingSave::Written,
    };
    assert_eq!(
        report.summary(),
        "Generated lighting: 2 lightmap(s), 0 light probe(s), 0 reflection probe(s); saved"
    );
    report.saved = LightingSave::NeedsSceneSave;
    assert!(report.summary().ends_with("; save the scene to keep it"));
}

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
    assert_eq!(refused.unwrap_err(), rusty_edit::LIGHTING_IN_PLAY);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "the file is untouched"
    );
}

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
    let (probes, lightmaps) = (scene.probes.probes().len(), scene.lightmaps.clone());
    assert!(probes > 0, "the bake placed probes");
    while scene.undo().is_some() {}
    assert_eq!(scene.probes.probes().len(), probes);
    assert_eq!(scene.lightmaps, lightmaps);
}
