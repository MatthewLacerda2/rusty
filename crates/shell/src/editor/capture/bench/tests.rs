//! On a primitive project in a temp dir: a floor with a baked navmesh, a camera and a
//! light probe. Without an adapter the round answers `None` and makes no rows up.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use glam::Vec3;
use rusty_components::CameraComponent;
use rusty_scene::create::{create_entity, Primitive};

use super::*;
use rusty_dev::bench::scenes::editor::EditorRound;

pub(super) fn project() -> (PathBuf, String) {
    let root = rusty_test_support::temp::dir().join(format!("editor_round_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let scenes = root.join("assets/scenes");
    std::fs::create_dir_all(&scenes).unwrap();
    let mut yard = Scene::new();
    let floor = create_entity(&mut yard, "Floor", Some(Primitive::Box));
    yard.world.transform_mut(floor).unwrap().scale = Vec3::new(10.0, 0.2, 10.0);
    let cam = yard.add_entity("Cam".into());
    yard.world.set_camera(cam, Some(CameraComponent::default()));
    yard.world
        .set_world_position(cam, Vec3::new(0.0, 2.0, -4.0));
    yard.probes.add_probe(Vec3::new(0.0, 1.0, 0.0));
    let navmesh = rusty_navigation::NavigationGraph::baked_bytes(&yard);
    yard.put_baked_navmesh(Arc::from(navmesh));
    let path = scenes.join("yard.scene").to_string_lossy().into_owned();
    rusty_edit::save_to_file(&yard, &path).unwrap();
    (root, path)
}

fn files(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut dirs = vec![dir.to_path_buf()];
    while let Some(d) = dirs.pop() {
        for entry in std::fs::read_dir(&d).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else {
                out.insert(path.clone(), std::fs::read(&path).unwrap());
            }
        }
    }
    out
}

/// Big enough that the editor's panels leave the Scene view room, as on a desktop window.
pub(super) const SIZE: (u32, u32) = (640, 360);

#[test]
fn gpu_the_editor_round_measures_frames_play_stop_and_save_and_writes_nothing() {
    let (root, scene) = project();
    let before = files(&root);
    let save_dir =
        rusty_test_support::temp::dir().join(format!("editor_save_{}", std::process::id()));
    let opts = Round {
        size: (SIZE.0, SIZE.1),
        warmup: 1,
        frames: 2,
        runs: 2,
        warm: false,
        switches: rusty_render::RenderSwitches::ON,
        save_dir: save_dir.clone(),
    };
    let mut game = editor_world(&scene, ProjectSettings::default()).unwrap();
    assert!(
        !game.resources.application.borrow().standalone(),
        "Play takes its snapshot"
    );
    let Some(round) = round(&mut game, &opts).unwrap() else {
        return;
    };
    has_its_rows(&round);
    assert!(
        !game.scene().borrow().is_playing(),
        "the round ends in edit mode"
    );
    assert_eq!(files(&root), before, "the project is read-only");
    assert!(!save_dir.exists(), "the save is deleted after");
}

/// Every row the round names, measured: the single samples with their spread
/// over the two runs, `play_rss_mb` the first run's alone.
fn has_its_rows(round: &EditorRound) {
    let row = |m: &str| round.rows.iter().find(|r| r.metric == m);
    let timed = [
        "editor.ui_ms",
        "editor.edit_ms",
        "play_enter_ms",
        "play_stop_ms",
        "save_ms",
    ];
    for metric in timed {
        let found = row(metric);
        assert!(
            found.is_some_and(|r| r.avg > 0.0),
            "{metric}: {:?}",
            round.rows
        );
    }
    let views = ["editor.render.handles", "editor.render.outline"];
    for metric in views
        .into_iter()
        .chain(rusty_dev::bench::scenes::editor::VIEW_ROWS)
    {
        assert!(row(metric).is_some(), "{metric}: {:?}", round.rows);
    }
    if cfg!(any(target_os = "linux", target_os = "macos")) {
        let rss = row("play_rss_mb").unwrap_or_else(|| panic!("{:?}", round.rows));
        assert_eq!(rss.lo, rss.hi, "the first run's alone: {rss:?}");
    }
    let enter = row("play_enter_ms").unwrap();
    assert!(enter.lo <= enter.avg && enter.avg <= enter.hi, "{enter:?}");
}

/// `write_atomic` renames a new file in, so a rewrite changes the inode.
fn inode(path: &Path) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path).ok().map(|m| m.ino())
}

#[test]
fn the_rounds_save_keeps_the_bake_files_as_a_save_of_the_loaded_scene_does() {
    let (_root, scene) = project();
    let game = editor_world(&scene, ProjectSettings::default()).unwrap();
    let dir = rusty_test_support::temp::dir().join(format!("editor_stage_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str| dir.join(name).to_string_lossy().into_owned();
    let navmesh = |path: &str| rusty_scene::nav_settings::navmesh_path(path);
    let world = game.scene().borrow();
    // A Save As to a path the world has no record of writes the bake there.
    rusty_edit::save_to_file(&world, &save("as.scene")).unwrap();
    assert!(navmesh(&save("as.scene")).exists(), "a Save As writes it");
    let staged = save("staged.scene");
    stage_save(&world, &scene, &staged).unwrap();
    let before = inode(&navmesh(&staged));
    assert!(before.is_some(), "the navmesh is copied beside the save");
    rusty_edit::save_to_file(&world, &staged).unwrap();
    assert_eq!(inode(&navmesh(&staged)), before, "kept, not rewritten");
    let occlusion = rusty_scene::lighting::occlusion::occlusion_path(&staged);
    assert!(!occlusion.exists(), "none loaded, none written");
    drop(world);
    let _ = std::fs::remove_dir_all(&dir);
}
