//! Light probes (#240): SH probe data model + analytic ambient sampling.
//!
//! Headless + deterministic. Asserts (1) probe POSITIONS round-trip through the
//! scene document while the baked SH round-trips through the `<scene>.lighting.json`
//! sidecar, and (2) a NON-UNIFORM analytic fill produces POSITION-DEPENDENT ambient
//! irradiance — the directional bounced light that replaces the old flat ambient
//! constant. No GPU; the analytic fill is pure scene-value math.

use glam::Vec3;
use rusty::scene::{analytic_fill, sidecar_path, Scene};

fn tmp(name: &str) -> String {
    std::env::temp_dir()
        .join(name)
        .to_string_lossy()
        .into_owned()
}

/// The whole-path acceptance: a height-varying analytic fill makes the sampled
/// ambient depend on POSITION, unlike the old constant hemispherical term.
#[test]
fn analytic_fill_is_position_dependent() {
    let mut scene = Scene::new();
    scene.ambient_color = Vec3::new(0.2, 0.3, 0.4);
    scene.ambient_intensity = 1.0;
    // A vertical column of probes so the fill varies with height.
    scene
        .probes
        .fill_grid(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 8.0, 0.0), 2.0);
    analytic_fill(&mut scene);

    // Sample the interpolated irradiance (up-facing normal) low vs high.
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
    // And the midpoint trilinearly interpolates between them.
    let mid = scene
        .probes
        .sample(Vec3::new(0.0, 4.0, 0.0))
        .unwrap()
        .eval(Vec3::Y);
    assert!(mid.x > low.x && mid.x < high.x, "mid={mid} not between");
}

/// Probe positions live in the scene; baked SH lives in the sidecar. Both survive a
/// save → load round-trip, and the SH is keyed by probe index.
#[test]
fn positions_in_scene_sh_in_sidecar_round_trip() {
    let mut scene = Scene::new();
    scene.ambient_color = Vec3::splat(0.5);
    scene.ambient_intensity = 1.0;
    scene
        .probes
        .fill_grid(Vec3::ZERO, Vec3::new(0.0, 4.0, 0.0), 2.0);
    analytic_fill(&mut scene);
    let probe_count = scene.probes.probes.len();
    let baked = scene.probes.probes[probe_count - 1].sh;

    let path = tmp("rusty_light_probes.scene");
    let sidecar = sidecar_path(&path);
    let _ = std::fs::remove_file(&sidecar);
    scene.save_to_file(&path).unwrap();

    // The scene doc carries positions but NOT the SH (sidecar holds it).
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
    loaded.load_from_file(&path).unwrap();
    assert_eq!(loaded.probes.probes.len(), probe_count);
    assert_eq!(
        loaded.probes.probes[0].position,
        scene.probes.probes[0].position
    );
    // SH merged back from the sidecar, index-aligned.
    assert_eq!(loaded.probes.probes[probe_count - 1].sh, baked);
}

/// A scene with placed-but-unbaked probes writes no sidecar (zero SH); load leaves
/// the probes at zero SH, which the shader treats as "no probe" fallback.
#[test]
fn unbaked_probes_write_no_sidecar() {
    let mut scene = Scene::new();
    scene.probes.add_probe(Vec3::ZERO);
    let path = tmp("rusty_light_probes_unbaked.scene");
    let sidecar = sidecar_path(&path);
    let _ = std::fs::remove_file(&sidecar);
    scene.save_to_file(&path).unwrap();
    assert!(
        !sidecar.exists(),
        "unbaked probes should not write a sidecar"
    );

    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    assert_eq!(loaded.probes.probes.len(), 1);
}
