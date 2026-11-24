//! `export --bake` and `--bake-resume`: the bake queue with no window, as a cron line or
//! an OS service runs it. Three primitive scenes at the lowest bake settings.

use std::path::{Path, PathBuf};
use std::process::Output;

use rusty::edit::authoring::{primitive_mesh_component, Primitive};
use rusty::scene::Scene;

const SCENES: [&str; 3] = ["a", "b", "c"];

fn project() -> PathBuf {
    let root = crate::temp::dir().join("game");
    std::fs::create_dir_all(root.join("assets/scenes")).unwrap();
    for name in SCENES {
        let mut scene = Scene::new();
        let floor = scene.add_entity("Floor".into());
        let plane = primitive_mesh_component(Primitive::Plane);
        scene.world.set_mesh(floor, plane);
        scene.world.set_static(floor, true);
        let s = &mut scene.lighting_settings.lightmaps;
        (s.texels_per_unit, s.samples, s.bounces, s.max_lightmap_size) = (1.0, 2, 1, 32);
        let file = root.join(format!("assets/scenes/{name}.scene"));
        rusty::edit::save_to_file(&scene, &file.to_string_lossy()).unwrap();
    }
    root
}

fn export(game: &Path, args: &[&str]) -> (Output, String) {
    let mut command = crate::spawn::engine_bin!("export");
    command.arg("--project").arg(game).args(args);
    let out = command.output().expect("the binary runs");
    eprintln!("{}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    (out, stdout)
}

fn baked(game: &Path, name: &str) -> bool {
    game.join(format!("assets/scenes/{name}.scene.lightmaps"))
        .is_dir()
}

#[test]
fn export_bakes_all_but_one_then_resumes_a_scene_a_kill_cut() {
    let game = project();
    let (out, stdout) = export(
        &game,
        &["--bake", "all", "--except", "assets/scenes/b.scene"],
    );
    assert!(out.status.success(), "{stdout}");
    assert!(
        stdout.contains("bake: assets/scenes/a.scene: done"),
        "{stdout}"
    );
    assert!(
        stdout.contains("bake: assets/scenes/c.scene: done"),
        "{stdout}"
    );
    assert!(baked(&game, "a") && baked(&game, "c") && !baked(&game, "b"));

    // A kill under b's first bake: the file still says `running`, the lock is free.
    let queue = game.join("cache/bake-queue.json");
    let cut = r#"{ "owner": 1, "scenes": [
        { "path": "assets/scenes/a.scene", "state": "done", "attempts": 1 },
        { "path": "assets/scenes/b.scene", "state": "running", "attempts": 1 } ] }"#;
    std::fs::write(&queue, cut).unwrap();
    let (out, stdout) = export(&game, &["--bake-resume"]);
    assert!(out.status.success(), "{stdout}");
    assert!(stdout.contains("b.scene: done"), "{stdout}");
    assert!(baked(&game, "b"), "the cut scene baked again");
    let (out, stdout) = export(&game, &["--bake-resume"]);
    assert!(
        out.status.success() && stdout.contains("nothing to resume"),
        "{stdout}"
    );
}

#[test]
fn export_exits_non_zero_when_a_scene_failed_after_baking_the_rest() {
    let game = project();
    let (out, stdout) = export(
        &game,
        &[
            "--bake",
            "assets/scenes/gone.scene",
            "assets/scenes/c.scene",
        ],
    );
    assert_eq!(out.status.code(), Some(1), "{stdout}");
    assert!(
        stdout.contains("gone.scene: failed (no scene file at"),
        "{stdout}"
    );
    assert!(stdout.contains("c.scene: done"), "{stdout}");
}
