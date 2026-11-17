//! [`bench`] end to end, on a project of two primitive scenes written to a temp
//! dir: one with a camera and a navmesh, one with neither. Renders when an adapter
//! is there (hence `gpu_`); without one the rendered rows are absent and the
//! memory and load rows still hold.

use glam::Vec3;
use rusty_components::CameraComponent;
use rusty_navigation::test_support::add_floor;
use rusty_scene::Scene;

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
    for (name, scene) in [("yard", &yard), ("empty", &empty)] {
        let path = scenes.join(format!("{name}.scene"));
        rusty_edit::save_to_file(scene, &path.to_string_lossy()).unwrap();
    }
    root
}

const OPTS: Options = Options {
    size: (64, 64),
    warmup: 1,
    frames: 2,
};

#[test]
fn gpu_a_projects_scenes_bench_from_their_viewpoints_with_load_and_memory() {
    let root = project();
    assert_eq!(project_scenes(&root), ["empty", "yard"]);
    let out = root.join("out");
    let settings = ProjectSettings::default;

    let yard = bench(&root, "yard", settings(), &out, OPTS).unwrap();
    assert_eq!(
        yard.views.len(),
        1 + 7 * views::YAWS.len(),
        "{:?}",
        yard.views
    );
    assert_eq!(yard.views[0], "camera Cam");
    let empty = bench(&root, "empty", settings(), &out, OPTS).unwrap();
    assert_eq!(empty.views, ["bounds centre"]);
    assert!(empty
        .warnings
        .iter()
        .any(|w| w.starts_with("no camera and no navmesh")));
    assert!(empty
        .warnings
        .iter()
        .any(|w| w.starts_with("not baked: lightmaps")));

    for report in [&yard, &empty] {
        assert!(report.row("frame_ms").is_some(), "{:?}", report.rows);
        assert!(report.row("load_ms").is_some_and(|r| r.avg > 0.0));
        if cfg!(any(target_os = "linux", target_os = "macos")) {
            assert!(report.row("rss_peak_mb").is_some_and(|r| r.avg > 0.0));
        }
        if report.row("renderer_ms").is_some()
            && cfg!(any(target_os = "linux", target_os = "macos"))
        {
            let gpu = report.row("gpu_mb");
            assert!(gpu.is_some_and(|r| r.avg > 0.0), "{:?}", report.rows);
        }
    }
    // Nothing was written into the project but the out dir asked for.
    let mut written: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    written.sort();
    assert_eq!(written, ["assets", "out"]);

    // The second run compares against the first, and picks the same viewpoints.
    let again = bench(&root, "yard", settings(), &out, OPTS).unwrap();
    assert_eq!(again.views, yard.views);
    let text = std::fs::read_to_string(out.join("yard.txt")).unwrap();
    assert!(!text.contains("no previous report"), "{text}");
}
