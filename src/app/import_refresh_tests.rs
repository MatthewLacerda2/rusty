//! Play refreshes before it holds (#1253): entering Play brings every converted
//! source under the project's `assets/` up to date, as Unity refreshes its assets
//! before Play, so a model only a script spawns has an import to play from. And
//! the hold Play takes ends with the world that took it.

use std::path::{Path, PathBuf};

use super::import_gate_tests::{stamp, world, QUAD};
use crate::asset::convert::{cached_gltf, conversion_allowed};
use crate::scene::ScriptComponent;

const DT: f32 = 1.0 / 60.0;

/// `<test>/assets/enemies/enemy.fbx`, a quad, never imported; returns the root.
fn assets() -> (PathBuf, PathBuf) {
    let root = crate::test_temp::dir().join("assets");
    std::fs::create_dir_all(root.join("enemies")).unwrap();
    let enemy = root.join("enemies/enemy.fbx");
    std::fs::write(&enemy, QUAD).unwrap();
    (root, enemy)
}

fn slashed(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[test]
fn a_model_only_a_script_spawns_loads_in_play() {
    let (root, enemy) = assets();
    let script = crate::test_temp::dir().join("spawner.lua");
    let code = format!(
        "return {{ Start = function(id)\n\
         local ok, e = pcall(Scene.Instantiate, '{}::Quad')\n\
         print(ok and 'spawned' or ('failed: ' .. tostring(e)))\n\
         end }}",
        slashed(&enemy)
    );
    std::fs::write(&script, code).unwrap();

    let mut gw = world();
    gw.resources.sources_root = root;
    {
        let mut scene = gw.world.scene.borrow_mut();
        let id = scene.add_entity("Spawner".to_string());
        *scene.world.scripts_mut(id).unwrap() = vec![ScriptComponent {
            path: slashed(&script),
            ..Default::default()
        }];
    }
    gw.set_playing(true);
    gw.tick(DT);

    let messages = &gw.console().borrow().messages;
    let said = |m: &str| messages.iter().any(|(line, _)| line == m);
    assert!(said("spawned"), "{messages:?}");
    assert!(
        !conversion_allowed(),
        "converted before the hold, not in Play"
    );
}

#[test]
fn a_world_dropped_in_play_releases_its_hold() {
    let (root, enemy) = assets();
    let mut gw = world();
    gw.resources.sources_root = root;
    gw.set_playing(true);
    gw.tick(DT);
    assert!(!conversion_allowed());
    let before = stamp(&enemy);
    drop(gw);

    assert!(conversion_allowed(), "no world plays");
    std::fs::write(&enemy, QUAD.replace("Quad", "Body")).unwrap();
    let mut other = world();
    other.tick(DT);
    cached_gltf(&enemy).unwrap();
    assert_ne!(stamp(&enemy), before, "another world converts");
}
