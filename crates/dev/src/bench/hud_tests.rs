//! The shared shooter HUD (`games/shared/`) on the stress yard, as `make bench` runs it:
//! the bench's copies are the source's bytes, and fed by `hud_feed.lua` it builds and
//! rewrites what a real game's HUD does.

use std::collections::BTreeMap;

use super::stress::{self, StressSpec};
use crate::harness::{fixture_project, Harness};

const SOURCE: &str = "games/shared";
const COPY: &str = "tests/fixtures/project/scenarios/bench";

#[test]
fn the_benchs_hud_is_the_shared_one() {
    let root = rusty_core::project::checkout();
    let shared = std::fs::read_dir(root.join(SOURCE)).unwrap();
    let mut files: Vec<_> = shared.map(|e| e.unwrap().file_name()).collect();
    files.sort();
    assert!(files.len() >= 2, "{files:?}");
    for file in files {
        let (source, copy) = (root.join(SOURCE).join(&file), root.join(COPY).join(&file));
        assert!(
            std::fs::read(&source).unwrap() == std::fs::read(&copy).unwrap_or_default(),
            "{} differs from its source {}: edit the source, then copy it over",
            copy.display(),
            source.display()
        );
    }
}

fn yard(name: &str) -> Harness {
    let out = rusty_test_support::temp::dir().join(format!("hud_{name}_{}", std::process::id()));
    let h = Harness::new(out, "");
    let spec = StressSpec {
        enemies: 4,
        lights: 0,
        flickering: 0,
        shadowed: 0,
        decals: 0,
        particle_systems: 0,
        ..StressSpec::default()
    };
    {
        let world = h.world.borrow();
        let mut scene = world.scene().borrow_mut();
        stress::load(&mut scene, &spec);
        let dir = fixture_project().join("scenarios/bench");
        let scripts = [
            ("HUD", "hud.lua"),
            ("HUD", "hud_cameras.lua"),
            ("HUD_Feed", "hud_feed.lua"),
        ];
        for (entity, file) in scripts {
            let id = (scene.find_entity_by_name(entity))
                .unwrap_or_else(|| scene.add_entity(entity.to_string()));
            let script = dir.join(file).to_string_lossy().into_owned();
            let added =
                rusty_edit::authoring::add_script(&mut scene.world, id, &script, BTreeMap::new());
            assert!(added, "{file}");
        }
    }
    h
}

#[test]
fn the_fed_hud_builds_its_ui_and_stacks_a_viewmodel() {
    let h = yard("fed");
    h.step(40);
    let world = h.world.borrow();
    let scene = world.scene().borrow();
    let id = |name: &str| (scene.find_entity_by_name(name)).unwrap_or_else(|| panic!("{name}"));
    let text = |name: &str| scene.world.text(id(name)).unwrap().text.clone();
    assert!(text("FPS").starts_with("60 FPS"), "{}", text("FPS"));
    assert_eq!(
        text("Ammo"),
        "20 / 60",
        "the 40th shot: 10 into the second magazine"
    );
    assert!(scene.world.image(id("Minimap")).is_some());

    let base = scene.world.camera(id("Camera")).unwrap().clone();
    let viewmodel = scene.world.camera(id("Viewmodel_Camera")).unwrap().clone();
    let layer = 1u32 << 30;
    assert_eq!(
        base.culling_mask & layer,
        0,
        "the world camera skips the gun"
    );
    assert_eq!(
        viewmodel.culling_mask, layer,
        "the viewmodel draws only the gun"
    );
    assert!(viewmodel.render_order > base.render_order);
    assert_eq!(
        scene.world.parent_id(id("Viewmodel_Camera")),
        Some(id("Camera"))
    );
    let shown = (1..=8)
        .map(|i| scene.world.text(id(&format!("Damage_{i}"))).unwrap())
        .filter(|t| t.color.w > 0.0)
        .count();
    assert!(shown > 0, "a damage number is floating");
    drop(scene);
    drop(world);
    let console = h.console_text();
    assert!(!console.contains("[ERROR]"), "{console}");
}
