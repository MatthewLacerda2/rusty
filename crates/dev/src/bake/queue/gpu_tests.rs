//! The queue over real bakes: three primitive scenes at the lowest settings.

use std::path::{Path, PathBuf};

use glam::Vec3;

use super::live::run_beside;
use super::{read, resolve, Read, Runner, Selection, State};
use crate::bake::lightmap::lightmap_dir;
use crate::bake::progress::BakeLog;
use rusty_edit::authoring::{primitive_mesh_component, Primitive};
use rusty_scene::Scene;

fn floor_scene(file: &Path) {
    let mut scene = Scene::new();
    let floor = scene.add_entity("Floor".into());
    let mesh = primitive_mesh_component(Primitive::Plane);
    scene.world.set_mesh(floor, mesh);
    scene.world.set_static(floor, true);
    let wall = scene.add_entity("Wall".into());
    scene
        .world
        .set_mesh(wall, primitive_mesh_component(Primitive::Box));
    scene.world.set_static(wall, true);
    if let Some(mut t) = scene.world.transform_mut(wall) {
        t.position = Vec3::new(0.0, 0.5, 0.0);
    }
    let s = &mut scene.lighting_settings.lightmaps;
    (s.texels_per_unit, s.samples, s.bounces, s.max_lightmap_size) = (1.0, 2, 1, 32);
    s.denoiser = rusty_scene::lighting::settings::Denoiser::None;
    rusty_edit::save_to_file(&scene, &file.to_string_lossy()).unwrap();
}

fn project(name: &str, scenes: &[&str]) -> PathBuf {
    let root = rusty_test_support::temp::dir().join(format!("bake_queue_gpu_{name}"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("assets/scenes")).unwrap();
    for name in scenes {
        floor_scene(&root.join("assets/scenes").join(name));
    }
    root
}

fn baked(root: &Path, scene: &str) -> bool {
    let file = root.join("assets/scenes").join(scene);
    lightmap_dir(&file.to_string_lossy()).is_dir()
}

fn run(root: &Path, selection: Selection) -> Vec<(String, State)> {
    let mut runner = Runner::start(root, resolve(root, &selection)).unwrap();
    let reopen = run_beside(&mut runner, &[], &mut |_| {}, &mut BakeLog::off()).unwrap();
    assert!(!reopen, "no scene was open");
    drop(runner);
    let Read::Found(queue) = read(root) else {
        panic!("the queue file was written")
    };
    queue
        .scenes
        .into_iter()
        .map(|e| (e.path, e.state))
        .collect()
}

#[test]
fn gpu_all_but_one_bakes_the_other_two_in_order() {
    let root = project("all", &["a.scene", "b.scene", "c.scene"]);
    let except = vec!["assets/scenes/b.scene".to_string()];
    let got = run(&root, Selection::All { except });
    let want = [
        ("assets/scenes/a.scene".to_string(), State::Done),
        ("assets/scenes/c.scene".to_string(), State::Done),
    ];
    assert_eq!(got, want);
    assert!(baked(&root, "a.scene") && baked(&root, "c.scene"));
    assert!(!baked(&root, "b.scene"), "the excepted scene is left alone");
}

#[test]
fn gpu_a_scene_that_cannot_bake_fails_and_the_next_still_bakes() {
    let root = project("broken", &["a.scene", "c.scene"]);
    std::fs::write(root.join("assets/scenes/b.scene"), "{ truncated").unwrap();
    let got = run(&root, Selection::All { except: vec![] });
    let states: Vec<State> = got.iter().map(|(_, s)| *s).collect();
    assert_eq!(states, [State::Done, State::Failed, State::Done]);
    let Read::Found(queue) = read(&root) else {
        panic!()
    };
    let why = queue.scenes[1].reason.clone().unwrap_or_default();
    assert!(!why.is_empty(), "the failure says why");
    assert!(baked(&root, "c.scene"));
}
