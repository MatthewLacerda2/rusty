use glam::Vec3;
use rusty_components::CameraComponent;
use rusty_navigation::test_support::add_floor;
use rusty_scene::Scene;
use std::path::Path;

use super::*;

fn project() -> std::path::PathBuf {
    let root = rusty_test_support::temp::dir().join(format!("scenes_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let scenes = root.join("assets/scenes");
    std::fs::create_dir_all(&scenes).unwrap();
    let mut yard = Scene::new();
    add_floor(&mut yard, -5.0, 5.0, -5.0, 5.0);
    let cam = yard.add_entity("Cam".into());
    yard.world.set_camera(cam, Some(CameraComponent::default()));
    yard.world
        .set_world_position(cam, Vec3::new(0.0, 2.0, -4.0));
    let empty = Scene::new();
    std::fs::create_dir_all(scenes.join("levels")).unwrap();
    for (name, scene) in [("yard", &yard), ("levels/empty", &empty)] {
        let path = scenes.join(format!("{name}.scene"));
        rusty_edit::save_to_file(scene, &path.to_string_lossy()).unwrap();
    }
    root
}

const OPTS: Options = Options {
    size: (64, 64),
    warmup: 1,
    frames: 2,
    runs: 2,
    warm: false,
    switches: rusty_render::RenderSwitches::ON,
};

fn labels(report: &SceneReport) -> Vec<&str> {
    report.views.iter().map(|k| k.label.as_str()).collect()
}

fn has_load_and_memory(report: &SceneReport) {
    assert!(report.row("frame_ms").is_some(), "{:?}", report.rows);
    assert!(report.row("load_ms").is_some_and(|r| r.avg > 0.0));
    if cfg!(any(target_os = "linux", target_os = "macos")) {
        assert!(report.row("rss_peak_mb").is_some_and(|r| r.avg > 0.0));
    }
    if report.row("renderer_ms").is_some() && cfg!(any(target_os = "linux", target_os = "macos")) {
        let gpu = report.row("gpu_mb");
        assert!(gpu.is_some_and(|r| r.avg > 0.0), "{:?}", report.rows);
        for k in ["submits", "render_passes", "hiz_tested", "cluster_culled"] {
            assert!(report.row(k).is_some(), "{k}: {:?}", report.rows);
        }
    }
}

#[test]
fn gpu_a_projects_scenes_bench_from_their_viewpoints_with_load_and_memory() {
    let root = project();
    assert_eq!(project_scenes(&root), ["levels/empty", "yard"]);
    let out = root.join("out");
    let settings = ProjectSettings::default;

    let extra = Extra::default();
    let yard = bench(&root, "yard", settings(), &out, OPTS, &extra).unwrap();
    assert_eq!(
        yard.views.len(),
        1 + 7 * views::YAWS.len(),
        "{:?}",
        yard.views
    );
    assert_eq!(labels(&yard)[0], "camera Cam");
    assert_eq!(
        yard.measured,
        2 * yard.views.len() as u32 * 2,
        "every frame of both runs"
    );
    let empty = bench(&root, "levels/empty", settings(), &out, OPTS, &extra).unwrap();
    assert_eq!(labels(&empty), ["bounds centre"]);
    assert!(empty
        .warnings
        .iter()
        .any(|w| w.starts_with("no camera, no spawn and no navmesh")));
    assert!(empty
        .warnings
        .iter()
        .any(|w| w.starts_with("not baked: lightmaps")));

    for report in [&yard, &empty] {
        has_load_and_memory(report);
    }
    let mut written: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    written.sort();
    assert_eq!(written, ["assets", "out"]);
    assert!(
        out.join("yard.console.log").is_file(),
        "the console is kept"
    );

    let again = bench(&root, "yard", settings(), &out, OPTS, &extra).unwrap();
    assert_eq!(again.views, yard.views);
    let text = std::fs::read_to_string(out.join("yard.txt")).unwrap();
    assert!(!text.contains("no previous report"), "{text}");
}

#[test]
fn unbaked_names_what_the_scene_places_but_never_loaded() {
    let mut scene = Scene::new();
    let lightmaps_only = "not baked: lightmaps (its lighting cost is not the shipped one)";
    assert_eq!(unbaked(&scene).as_deref(), Some(lightmaps_only));
    scene.probes.add_probe(Vec3::ZERO);
    scene.probes.add_probe(Vec3::X);
    let w = unbaked(&scene).unwrap();
    assert!(
        w.contains("light probes (placed, no baked SH loaded)"),
        "{w}"
    );
    for p in scene.probes.probes_mut() {
        p.sh.coeffs[0] = [0.5; 3];
    }
    assert_eq!(unbaked(&scene).as_deref(), Some(lightmaps_only));
    scene.reflection_probes.add(Vec3::ZERO, Vec3::ONE);
    let w = unbaked(&scene).unwrap();
    assert!(
        w.contains("reflection probes (1 of 1 with no cube on disk)"),
        "{w}"
    );
}

#[test]
fn a_projects_key_is_its_name_and_a_hash_of_where_it_lies() {
    let a = out_key(Path::new("/home/me/experiments/infinity-shout"));
    let b = out_key(Path::new("/home/me/games/infinity-shout"));
    assert!(a.starts_with("infinity-shout-") && b.starts_with("infinity-shout-"));
    assert_ne!(a, b, "a copy and its original keep their own history");
    assert_eq!(a, out_key(Path::new("/home/me/experiments/infinity-shout")));
}
