//! SH probe data model + analytic ambient sampling: positions round-trip through the scene document, baked SH through the `<scene>.lighting.json` sidecar, and a non-uniform analytic fill gives position-dependent ambient. No GPU.

use crate::temp::path as tmp;
use glam::Vec3;
use rusty::scene::{analytic_fill, sidecar_path, Scene};

#[test]
fn analytic_fill_is_position_dependent() {
    let mut scene = Scene::new();
    scene.ambient_color = Vec3::new(0.2, 0.3, 0.4);
    scene.ambient_intensity = 1.0;
    scene
        .probes
        .fill_grid(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 8.0, 0.0), 2.0);
    analytic_fill(&mut scene);

    let low = scene
        .probes
        .sample(Vec3::new(0.0, 0.0, 0.0))
        .unwrap()
        .eval(Vec3::Y);
    let high = scene
        .probes
        .sample(Vec3::new(0.0, 8.0, 0.0))
        .unwrap()
        .eval(Vec3::Y);
    // Position-dependent: the old flat ambient would return the SAME value here.
    assert!(
        high.x > low.x + 1.0e-3,
        "ambient should vary with position: low={low} high={high}"
    );
    let mid = scene
        .probes
        .sample(Vec3::new(0.0, 4.0, 0.0))
        .unwrap()
        .eval(Vec3::Y);
    assert!(mid.x > low.x && mid.x < high.x, "mid={mid} not between");
}

#[test]
fn positions_in_scene_sh_in_sidecar_round_trip() {
    let mut scene = Scene::new();
    scene.ambient_color = Vec3::splat(0.5);
    scene.ambient_intensity = 1.0;
    scene
        .probes
        .fill_grid(Vec3::ZERO, Vec3::new(0.0, 4.0, 0.0), 2.0);
    analytic_fill(&mut scene);
    let probe_count = scene.probes.probes().len();
    let baked = scene.probes.probes()[probe_count - 1].sh;

    let path = tmp("rusty_light_probes.scene");
    let sidecar = sidecar_path(&path);
    let _ = std::fs::remove_file(&sidecar);
    rusty::edit::save_to_file(&scene, &path).unwrap();

    let scene_json = std::fs::read_to_string(&path).unwrap();
    assert!(
        scene_json.contains("\"probes\""),
        "positions missing from scene"
    );
    assert!(
        !scene_json.contains("\"coeffs\""),
        "baked SH leaked into the scene document"
    );
    assert!(sidecar.exists(), "lighting sidecar was not written");

    let mut loaded = Scene::new();
    rusty::edit::load_from_file(&mut loaded, &path).unwrap();
    assert_eq!(loaded.probes.probes().len(), probe_count);
    assert_eq!(
        loaded.probes.probes()[0].position,
        scene.probes.probes()[0].position
    );
    assert_eq!(loaded.probes.probes()[probe_count - 1].sh, baked);
}

/// Placed-but-unbaked probes write no sidecar; load leaves them at zero SH, which the shader treats as no probe.
#[test]
fn unbaked_probes_write_no_sidecar() {
    let mut scene = Scene::new();
    scene.probes.add_probe(Vec3::ZERO);
    let path = tmp("rusty_light_probes_unbaked.scene");
    let sidecar = sidecar_path(&path);
    let _ = std::fs::remove_file(&sidecar);
    rusty::edit::save_to_file(&scene, &path).unwrap();
    assert!(
        !sidecar.exists(),
        "unbaked probes should not write a sidecar"
    );

    let mut loaded = Scene::new();
    rusty::edit::load_from_file(&mut loaded, &path).unwrap();
    assert_eq!(loaded.probes.probes().len(), 1);
}
