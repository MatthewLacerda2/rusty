//! A scene benched without the baked data it has on disk says so, one line a kind:
//! a `.occlusion` its load refused, a PVS its statics no longer matched at load, a
//! navmesh Play's start refused; a scene whose bakes are current says nothing, even
//! when the game then moves a baked static.

use glam::Vec3;
use rusty_components::CameraComponent;
use rusty_lighting::occlusion::bake_occlusion;
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::lighting::occlusion::{encode, occlusion_path};
use rusty_scene::Scene;

use super::*;

const OPTS: Options = Options {
    size: (32, 32),
    warmup: 1,
    frames: 1,
    runs: 1,
    warm: false,
    switches: rusty_render::RenderSwitches::ON,
};

/// A static wall between a camera and a box, the wall at `wall_x`.
fn walled(wall_x: f32) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let wall = create_entity(&mut scene, "Wall", Some(Primitive::Box));
    let behind = create_entity(&mut scene, "Behind", Some(Primitive::Box));
    for (id, at, scale) in [
        (wall, Vec3::new(wall_x, 1.0, 0.0), Vec3::new(8.0, 4.0, 0.5)),
        (behind, Vec3::new(0.0, 0.5, 4.0), Vec3::ONE),
    ] {
        scene.world.set_static(id, true);
        let mut t = scene.world.transform_mut(id).unwrap();
        (t.position, t.scale) = (at, scale);
    }
    let cam = scene.add_entity("Cam".into());
    scene
        .world
        .set_camera(cam, Some(CameraComponent::default()));
    scene
        .world
        .set_world_position(cam, Vec3::new(0.0, 1.0, -4.0));
    scene
}

/// The folder of the project `name`.
fn root(name: &str) -> std::path::PathBuf {
    rusty_test_support::temp::dir().join(format!("baked_{name}"))
}

/// A project of `scene` as `walls.scene`, beside `pvs` as its `.occlusion`.
fn project(name: &str, scene: &Scene, pvs: &[u8]) -> std::path::PathBuf {
    let root = root(name);
    let _ = std::fs::remove_dir_all(&root);
    let scenes = root.join("assets/scenes");
    std::fs::create_dir_all(&scenes).unwrap();
    let path = scenes.join("walls.scene").to_string_lossy().into_owned();
    rusty_edit::save_to_file(scene, &path).unwrap();
    std::fs::write(occlusion_path(&path), pvs).unwrap();
    root
}

fn baked(root: &Path) -> Vec<String> {
    let (settings, out) = (ProjectSettings::default(), root.join("out"));
    let report = bench(root, "walls", settings, &out, OPTS, &Extra::default());
    report
        .unwrap()
        .warnings
        .into_iter()
        .filter(|w| w.starts_with("baked "))
        .collect()
}

fn pvs() -> Vec<u8> {
    encode(
        &bake_occlusion(&walled(0.0), 1)
            .0
            .expect("the wall occludes"),
    )
}

#[test]
fn gpu_a_current_pvs_says_nothing_and_a_refused_one_says_why() {
    assert_eq!(
        baked(&project("current", &walled(0.0), &pvs())),
        Vec::<String>::new()
    );
    let refused = baked(&project("refused", &walled(0.0), b"not a pvs"));
    assert_eq!(refused.len(), 1, "{refused:?}");
    assert!(
        refused[0].starts_with("baked occlusion not used: walls.scene.occlusion refused ("),
        "{refused:?}"
    );
}

#[test]
fn gpu_a_pvs_whose_occluder_moved_since_says_stale() {
    let stale = baked(&project("stale", &walled(3.0), &pvs()));
    // The renderer judges it: with no adapter, nothing is drawn and nothing judged.
    let Some(line) = stale.first() else {
        return assert!(!rusty_render::gpu_required(), "no line");
    };
    assert!(
        line.starts_with("baked occlusion not used: walls.scene.occlusion stale (1 of the 1 "),
        "{stale:?}"
    );
}

#[test]
fn gpu_a_navmesh_plays_start_refused_says_so() {
    let root = project("navmesh", &walled(0.0), &pvs());
    let scene = root.join("assets/scenes/walls.scene");
    let navmesh = rusty_scene::nav_settings::navmesh_path(&scene.to_string_lossy());
    std::fs::write(navmesh, b"not a navmesh").unwrap();
    let lines = baked(&root);
    assert_eq!(lines.len(), 1, "{lines:?}");
    let refused = "baked navmesh not used: walls.scene.navmesh refused at Play's start (";
    assert!(lines[0].starts_with(refused), "{lines:?}");
}

/// What the mover script does: from its second `Update` (after the load's first
/// frame), the wall it sits on is moved off the bake.
const MOVER: &str = "local Mover = {}
local ticks = 0
function Mover.Update(id, dt)
    ticks = ticks + 1
    if ticks > 1 then Transform.SetPosition(id, 3, 1, 0) end
end
return Mover
";

#[test]
fn gpu_a_static_the_game_moves_during_play_is_no_stale_bake() {
    let script = root("moved").join("assets/mover.lua");
    let mut scene = walled(0.0);
    let wall = scene.find_entity_by_name("Wall").expect("the wall");
    let (path, fields) = (script.to_string_lossy(), Default::default());
    assert!(rusty_edit::authoring::add_script(
        &mut scene.world,
        wall,
        &path,
        fields
    ));
    let root = project("moved", &scene, &pvs());
    std::fs::write(&script, MOVER).unwrap();
    assert_eq!(baked(&root), Vec::<String>::new());
}
